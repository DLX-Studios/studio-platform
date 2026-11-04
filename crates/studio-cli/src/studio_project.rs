//! File-routed Studio projects shared by source checks, development and packaging.
use crate::build::{BuildDiagnostic, BuildFailure, BuildPhase};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use studio_navigation::project::{ProjectRoute, RoutingManifest, ROUTING_ASSET};
use studio_navigation::timers::{TimerManifest, TimerPage, TIMER_ASSET};
use studio_protocol::dev_rpc::DevSourceLocation;
use studio_script::ir::StudioModule;

/// Generated source links for expanded component instances in the native preview.
pub const SOURCE_MAP_ASSET: &str = "assets/studio/source-map.json";

/// One composed source project and its host-admitted route declaration.
pub struct RoutedProject {
    /// Composed components, one per route.
    pub module: StudioModule,
    /// Deterministic route declaration.
    pub routing: RoutingManifest,
    /// Actual source links for each page's expanded node tree.
    pub source_locations: BTreeMap<String, BTreeMap<String, DevSourceLocation>>,
    /// Compiler-declared, host-scheduled page timers.
    pub timers: TimerManifest,
}

impl RoutedProject {
    pub(crate) fn single(
        composed: studio_script::project::ProjectCompilation,
        source: String,
        id: String,
    ) -> Self {
        let locations = composed
            .source_locations
            .into_iter()
            .map(|(node, location)| {
                (
                    node,
                    DevSourceLocation {
                        path: location.path,
                        line: location.span.start.line as u64,
                        column: location.span.start.column as u64,
                        offset: location.span.start.offset as u64,
                    },
                )
            })
            .collect();
        let pages = if composed.timers.is_empty() {
            Vec::new()
        } else {
            vec![TimerPage {
                id: id.clone(),
                timers: composed.timers,
            }]
        };
        Self {
            module: composed.module,
            routing: RoutingManifest {
                schema_version: 1,
                routes: vec![ProjectRoute {
                    path: format!("/{id}"),
                    id: id.clone(),
                    source,
                }],
            },
            source_locations: BTreeMap::from([(id, locations)]),
            timers: TimerManifest {
                schema_version: 1,
                pages,
            },
        }
    }

    /// Stage compiler-owned routing and source links without editing the authored manifest.
    ///
    /// # Errors
    /// Returns a bounded metadata encoding error.
    pub fn assets(&self) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
        let mut assets = BTreeMap::new();
        let routing =
            serde_json::to_vec(&self.routing).map_err(|error| failure(error.to_string()))?;
        RoutingManifest::from_bytes(&routing).map_err(failure)?;
        let locations = serde_json::to_vec(&self.source_locations)
            .map_err(|error| failure(error.to_string()))?;
        if locations.len() > 1024 * 1024 {
            return Err(failure("expanded source map exceeds 1 MiB"));
        }
        assets.insert(ROUTING_ASSET.to_owned(), routing);
        assets.insert(SOURCE_MAP_ASSET.to_owned(), locations);
        if self.timers.pages.iter().any(|page| !page.timers.is_empty()) {
            let bytes =
                serde_json::to_vec(&self.timers).map_err(|error| failure(error.to_string()))?;
            TimerManifest::from_bytes(&bytes).map_err(failure)?;
            assets.insert(TIMER_ASSET.to_owned(), bytes);
        }
        Ok(assets)
    }
}

/// Whether a project declares `.studio` page sources, including currently-invalid declarations.
#[must_use]
pub fn has_file_routes(root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let routes = root.join("routes");
    if !routes.is_dir() {
        return false;
    }
    let mut pending = vec![routes];
    let mut visited = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        let Ok(canonical) = directory.canonicalize() else {
            // Let authoritative discovery report the invalid declaration rather
            // than silently selecting an unrelated legacy entry point.
            return true;
        };
        if !canonical.starts_with(&root) {
            return true;
        }
        if !visited.insert(canonical) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return true;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return true;
            };
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('_') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                // Canonical containment is checked before reading that directory.
                // Mode detection never reads any source-file contents.
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "studio")
                && name != "+layout.studio"
            {
                return true;
            }
        }
    }
    false
}

/// Reject authored files or manifest entries that collide with generated metadata.
/// Call this only when the current compiler path will stage generated metadata.
pub(crate) fn ensure_generated_assets_available(root: &Path) -> Result<(), BuildFailure> {
    let declared = std::fs::read(root.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|manifest| {
            manifest
                .get("assets")
                .and_then(serde_json::Value::as_array)
                .cloned()
        });
    for key in [ROUTING_ASSET, SOURCE_MAP_ASSET, TIMER_ASSET] {
        let authored = declared
            .as_ref()
            .is_some_and(|assets| assets.iter().any(|asset| asset.as_str() == Some(key)));
        // symlink_metadata also rejects broken authored links at a reserved key.
        if std::fs::symlink_metadata(root.join(key)).is_ok() || authored {
            return Err(failure(format!(
                "`{key}` is reserved for compiler-generated metadata"
            )));
        }
    }
    Ok(())
}

/// Compile file routes, returning None for legacy projects.
///
/// # Errors
/// Returns source-linked discovery or composition errors; no partial project is returned.
pub fn compile_routes(root: &Path) -> Result<Option<RoutedProject>, BuildFailure> {
    if root.join("studio.project.json").exists() || !has_file_routes(root) {
        return Ok(None);
    }
    ensure_generated_assets_available(root)?;
    let registry = crate::routes::build_registry(root).map_err(|errors| {
        BuildFailure::new(
            BuildPhase::Compile,
            errors
                .into_iter()
                .map(|error| {
                    BuildDiagnostic::error(
                        BuildPhase::Compile,
                        error.code,
                        format!("{}: {}", error.files.join(", "), error.message),
                    )
                })
                .collect(),
        )
    })?;
    if registry.entries.len() > studio_navigation::project::MAX_PROJECT_ROUTES {
        return Err(failure("routed projects support at most 64 pages"));
    }
    if registry
        .entries
        .iter()
        .any(|entry| !entry.file.ends_with(".studio"))
    {
        return Err(failure("a routed Studio project must use .studio pages consistently; keep legacy .ts route projects separate"));
    }
    let mut module = StudioModule {
        version: studio_script::ir::STUDIO_IR_VERSION,
        id: "studio-file-project".into(),
        imports: Vec::new(),
        exports: Vec::new(),
        components: Vec::new(),
        dependencies: Vec::new(),
    };
    let mut routing = RoutingManifest {
        schema_version: 1,
        routes: Vec::new(),
    };
    let mut source_locations = BTreeMap::new();
    let mut timers = TimerManifest {
        schema_version: 1,
        pages: Vec::new(),
    };
    for entry in registry.entries {
        let digest = Sha256::digest(entry.path.as_bytes());
        let id = format!(
            "page_{}",
            digest[..8]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let layouts = ancestor_layouts(root, Path::new(&entry.file));
        let composed =
            studio_script::project::compile_component(root, Path::new(&entry.file), &layouts, &id)
                .map_err(|errors| {
                    BuildFailure::new(
                        BuildPhase::Compile,
                        errors
                            .into_iter()
                            .map(|error| {
                                BuildDiagnostic::error(
                                    BuildPhase::Compile,
                                    error.diagnostic.code,
                                    format!("{}: {}", error.path, error.diagnostic.message),
                                )
                                .at(
                                    error.diagnostic.span.start.line as u64,
                                    error.diagnostic.span.start.column as u64,
                                )
                            })
                            .collect(),
                    )
                })?;
        source_locations.insert(
            id.clone(),
            composed
                .source_locations
                .into_iter()
                .map(|(node, location)| {
                    (
                        node,
                        DevSourceLocation {
                            path: location.path,
                            line: location.span.start.line as u64,
                            column: location.span.start.column as u64,
                            offset: location.span.start.offset as u64,
                        },
                    )
                })
                .collect(),
        );
        if !composed.timers.is_empty() {
            timers.pages.push(TimerPage {
                id: id.clone(),
                timers: composed.timers,
            });
        }
        module.components.extend(composed.module.components);
        module.dependencies.extend(composed.module.dependencies);
        module.exports.push(id.clone());
        routing.routes.push(ProjectRoute {
            id,
            path: entry.path,
            source: entry.file,
        });
    }
    routing.validate().map_err(failure)?;
    routing.initial_route().map_err(failure)?;
    module.dependencies.sort();
    module.dependencies.dedup();
    Ok(Some(RoutedProject {
        module,
        routing,
        source_locations,
        timers,
    }))
}

fn ancestor_layouts(root: &Path, page: &Path) -> Vec<PathBuf> {
    let mut ancestors = Vec::new();
    let mut directory = page.parent();
    while let Some(path) = directory {
        if !path.starts_with("routes") {
            break;
        }
        let layout = path.join("+layout.studio");
        if root.join(&layout).exists() {
            ancestors.push(layout);
        }
        directory = path.parent();
    }
    ancestors.reverse();
    ancestors
}

fn failure(message: impl Into<String>) -> BuildFailure {
    BuildFailure::new(
        BuildPhase::Compile,
        vec![BuildDiagnostic::error(
            BuildPhase::Compile,
            "STUDIO340",
            message,
        )],
    )
}
