//! Typed lucide asset collection: oxc-parsed reference graph, staged output,
//! and budget enforcement.
//!
//! References are `iconNode("id", "name")` and `Icon("id", "name")` call
//! expressions with two string literals, found by parsing — never by pattern
//! matching. Renames, member calls, comments, and non-literals are not
//! references by construction.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;

/// Documented ceiling for total collected icon bytes.
pub const ICON_BUDGET_BYTES: u64 = 512 * 1024;

/// One icon reference site.
#[derive(Clone, Debug, PartialEq)]
pub struct IconSpan {
    /// Source file.
    pub file: PathBuf,
    /// One-based line number.
    pub line: u64,
    /// One-based column number.
    pub column: u64,
}

/// One referenced icon with every referencing span.
#[derive(Clone, Debug, PartialEq)]
pub struct IconReference {
    /// Icon name, case-sensitive.
    pub name: String,
    /// Referencing spans in file order.
    pub spans: Vec<IconSpan>,
}

/// One resolved icon: bytes plus override provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct IconBytes {
    /// File bytes shipped.
    pub bytes: Vec<u8>,
    /// True when a committed `assets/icons/` file won over the package.
    pub overridden: bool,
}

/// Resolved collection: names in sorted order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IconCollection {
    /// Resolved icons by name.
    pub entries: BTreeMap<String, IconBytes>,
}

/// Asset collection failure with stable codes.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetFailure {
    /// `BUILD_ASSET_ICON_MISSING`, `BUILD_ASSET_BUDGET_EXCEEDED`, or
    /// `BUILD_ASSET_PACKAGE_MISSING`.
    pub code: &'static str,
    /// Safe message.
    pub message: String,
    /// Referencing spans for missing icons.
    pub spans: Vec<IconSpan>,
}

/// Collect icon references from every `.ts` file under `assembly/`.
#[must_use]
pub fn collect_references(project: &Path) -> BTreeMap<String, IconReference> {
    let mut references: BTreeMap<String, IconReference> = BTreeMap::new();
    let assembly = project.join("assembly");
    let mut files = Vec::new();
    if assembly.is_dir() {
        let mut stack = vec![assembly];
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "ts") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    for file in files {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (name, span) in scan_file(&file, &source) {
            references
                .entry(name.clone())
                .or_insert_with(|| IconReference {
                    name,
                    spans: Vec::new(),
                })
                .spans
                .push(span);
        }
    }
    references
}

struct ReferenceVisitor<'a> {
    file: &'a Path,
    lines: &'a [usize],
    found: Vec<(String, IconSpan)>,
}

impl<'a> Visit<'a> for ReferenceVisitor<'a> {
    fn visit_call_expression(&mut self, node: &CallExpression<'a>) {
        if !matches!(&node.callee, Expression::Identifier(ident) if ident.name == "iconNode" || ident.name == "Icon")
        {
            self.visit_expression(&node.callee);
            for argument in node.arguments.iter() {
                self.visit_argument(argument);
            }
            return;
        }
        let mut arguments = node.arguments.iter();
        let (Some(_id), Some(icon)) = (arguments.next(), arguments.next()) else {
            self.visit_expression(&node.callee);
            for argument in node.arguments.iter() {
                self.visit_argument(argument);
            }
            return;
        };
        if let Argument::StringLiteral(literal) = icon {
            let span = literal.span;
            let (line, column) = line_column(self.lines, span.start);
            self.found.push((
                literal.value.to_string(),
                IconSpan {
                    file: self.file.to_path_buf(),
                    line,
                    column,
                },
            ));
        }
        self.visit_expression(&node.callee);
        for argument in node.arguments.iter() {
            self.visit_argument(argument);
        }
    }
}

/// Walk one file's AST for icon references.
fn scan_file(file: &Path, source: &str) -> Vec<(String, IconSpan)> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file).unwrap_or_default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return Vec::new();
    }
    let lines = line_starts(source);
    let mut visitor = ReferenceVisitor {
        file,
        lines: &lines,
        found: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    visitor.found
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn line_column(starts: &[usize], offset: u32) -> (u64, u64) {
    let offset = offset as usize;
    let line = starts
        .partition_point(|start| *start <= offset)
        .saturating_sub(1);
    let column = offset.saturating_sub(starts[line.min(starts.len() - 1)]) + 1;
    (
        u64::try_from(line + 1).unwrap_or(u64::MAX),
        u64::try_from(column).unwrap_or(u64::MAX),
    )
}

/// Resolve references against the pinned package with override precedence.
///
/// Committed `assets/icons/<name>.svg` files win silently; otherwise the
/// package file ships byte-identical. Missing names and budget overruns fail.
///
/// # Errors
///
/// Returns [`AssetFailure`] for missing packages, missing icons, or budget
/// overruns.
pub fn resolve_collection(
    project: &Path,
    references: &BTreeMap<String, IconReference>,
    package_icons: &Path,
) -> Result<IconCollection, AssetFailure> {
    if !references.is_empty() && !package_icons.is_dir() {
        return Err(AssetFailure {
            code: "BUILD_ASSET_PACKAGE_MISSING",
            message: format!(
                "lucide package not installed at {} (run `bun install`)",
                package_icons.display()
            ),
            spans: Vec::new(),
        });
    }
    let mut collection = IconCollection::default();
    let mut total: u64 = 0;
    for reference in references.values() {
        let file_name = format!("{}.svg", reference.name);
        let override_path = project.join("assets/icons").join(&file_name);
        let (bytes, overridden) = if override_path.is_file() {
            (
                std::fs::read(&override_path).map_err(|error| AssetFailure {
                    code: "BUILD_ASSET_PACKAGE_MISSING",
                    message: format!("read override {}: {error}", override_path.display()),
                    spans: Vec::new(),
                })?,
                true,
            )
        } else {
            let packaged = package_icons.join(&file_name);
            if !packaged.is_file() {
                return Err(AssetFailure {
                    code: "BUILD_ASSET_ICON_MISSING",
                    message: format!(
                        "icon `{}` is not in the pinned lucide package",
                        reference.name
                    ),
                    spans: reference.spans.clone(),
                });
            }
            (
                std::fs::read(&packaged).map_err(|error| AssetFailure {
                    code: "BUILD_ASSET_PACKAGE_MISSING",
                    message: format!("read {}: {error}", packaged.display()),
                    spans: Vec::new(),
                })?,
                false,
            )
        };
        total += bytes.len() as u64;
        collection
            .entries
            .insert(reference.name.clone(), IconBytes { bytes, overridden });
    }
    if total > ICON_BUDGET_BYTES {
        return Err(AssetFailure {
            code: "BUILD_ASSET_BUDGET_EXCEEDED",
            message: format!(
                "collected icons use {total} bytes, over the {ICON_BUDGET_BYTES}-byte ceiling"
            ),
            spans: Vec::new(),
        });
    }
    Ok(collection)
}

/// Stage collected icons under `build/staging/icons/` for packaging.
pub fn stage_collection(
    project: &Path,
    collection: &IconCollection,
) -> std::io::Result<BTreeMap<String, PathBuf>> {
    let staging = project.join("build/staging/icons");
    std::fs::create_dir_all(&staging)?;
    let mut staged = BTreeMap::new();
    for (name, icon) in &collection.entries {
        let path = staging.join(format!("{name}.svg"));
        let existing = std::fs::read(&path).ok();
        if existing.as_deref() != Some(icon.bytes.as_slice()) {
            std::fs::write(&path, &icon.bytes)?;
        }
        staged.insert(format!("assets/icons/{name}.svg"), path);
    }
    Ok(staged)
}
