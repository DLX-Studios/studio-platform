//! Bounded composition of local Studio components and route layouts.
//!
//! Imports resolve on disk before the ordinary portable-subset compiler runs.
//! Composition operates on typed IR; guest JavaScript is never evaluated.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use studio_navigation::timers::TimerSpec;

use crate::ir::{
    ComponentDefinition, ContentBinding, PropValue, StudioExpression, StudioModule, TemplateNode,
};
use crate::rsvelte_adapter::{
    AdapterEngine, SourceAttribute, SourceAttributePart, SourceAttributeValue, SourceExpression,
    SourceModule, SourceNode, SourceSpan, span_for,
};
use crate::types::{StudioType, StudioValue};
use crate::{CODE_MODULE_GRAPH, CODE_SHAPE, Diagnostic, Severity, Span};

const MAX_MODULES: usize = 64;
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_INSTANCES: usize = 1024;
const MAX_NODES: usize = 10_000;

/// A source diagnostic retaining the actual project-relative file.
#[derive(Debug, Clone)]
pub struct ProjectDiagnostic {
    /// Source path relative to the canonical project root.
    pub path: String,
    /// Existing Studio compiler diagnostic.
    pub diagnostic: Diagnostic,
}

/// The authored source of one composed node.
#[derive(Debug, Clone)]
pub struct ProjectSourceLocation {
    /// Project-relative source file.
    pub path: String,
    /// Source span in that file.
    pub span: Span,
}

/// One fully composed entry shared by dev evaluation and production lowering.
#[derive(Debug, Clone)]
pub struct ProjectCompilation {
    /// One flattened component with scoped instance state and node identities.
    pub module: StudioModule,
    /// Node-to-authored-source mapping, including imported components and layouts.
    pub source_locations: BTreeMap<String, ProjectSourceLocation>,
    /// Bounded source-only timers with their fully scoped native target IDs.
    pub timers: Vec<TimerSpec>,
}

/// Compile a page and its ordered ancestor layouts from local `.studio` sources.
///
/// `entry` and `layouts` are project-relative paths; layouts are ordered outermost first.
/// Imports support only static default imports of local `.studio` components. The
/// default layout/child outlet is `<Slot />` or `{@render children()}`.
///
/// # Errors
/// Returns source-linked diagnostics for invalid source, imports, bounds, props,
/// cycles, or canonical paths outside the project. No partial module is returned.
pub fn compile_component(
    project_root: &Path,
    entry: &Path,
    layouts: &[PathBuf],
    name: &str,
) -> Result<ProjectCompilation, Vec<ProjectDiagnostic>> {
    let root = project_root
        .canonicalize()
        .map_err(|error| vec![issue("", format!("project root: {error}"), empty_span())])?;
    let mut loader = Loader {
        root,
        modules: BTreeMap::new(),
        loading: Vec::new(),
        source_bytes: 0,
        instances: 0,
        nodes: 0,
    };
    let entry = loader.resolve_entry(entry)?;
    loader.load(&entry)?;
    let mut composed = loader.instantiate(&entry, "", None, None, 0)?;
    for (index, layout) in layouts.iter().enumerate().rev() {
        let path = loader.resolve_entry(layout)?;
        loader.load(&path)?;
        let layout_children = std::mem::take(&mut composed.component.template);
        let mut wrapper = loader.instantiate(
            &path,
            &format!("layout{index}"),
            None,
            Some(&layout_children),
            0,
        )?;
        merge_public_props(
            &path,
            &mut wrapper.component.props,
            std::mem::take(&mut composed.component.props),
        )?;
        wrapper
            .component
            .state
            .append(&mut composed.component.state);
        wrapper
            .component
            .derived
            .append(&mut composed.component.derived);
        wrapper
            .component
            .handlers
            .append(&mut composed.component.handlers);
        wrapper.locations.append(&mut composed.locations);
        wrapper.timers.append(&mut composed.timers);
        composed = wrapper;
    }
    name.clone_into(&mut composed.component.id);
    name.clone_into(&mut composed.component.name);
    composed.component.content = collect_content(&composed.component.template);
    for slot in &mut composed.component.derived {
        let mut dependencies = BTreeSet::new();
        collect_dependencies(&slot.expression, &mut dependencies);
        slot.dependencies = dependencies.into_iter().collect();
    }
    validate_composed(&composed.component, &composed.locations, &entry)?;
    validate_timer_targets(&composed.component, &composed.timers, &entry)?;
    let dependencies: Vec<String> = loader.modules.keys().cloned().collect();
    Ok(ProjectCompilation {
        module: StudioModule {
            version: crate::ir::STUDIO_IR_VERSION,
            id: name.to_owned(),
            imports: Vec::new(),
            exports: vec![name.to_owned()],
            components: vec![composed.component],
            dependencies,
        },
        source_locations: composed.locations,
        timers: composed.timers,
    })
}

#[derive(Clone)]
struct Loaded {
    component: ComponentDefinition,
    imports: BTreeMap<String, String>,
    outlets: usize,
    timers: Vec<TimerSpec>,
}

struct Composed {
    component: ComponentDefinition,
    locations: BTreeMap<String, ProjectSourceLocation>,
    timers: Vec<TimerSpec>,
}

struct Expansion<'a> {
    path: &'a str,
    imports: &'a BTreeMap<String, String>,
    target: &'a mut ComponentDefinition,
    locations: &'a mut BTreeMap<String, ProjectSourceLocation>,
    timers: &'a mut Vec<TimerSpec>,
    slot: Option<&'a [TemplateNode]>,
}

struct Loader {
    root: PathBuf,
    modules: BTreeMap<String, Loaded>,
    loading: Vec<String>,
    source_bytes: usize,
    instances: usize,
    nodes: usize,
}

impl Loader {
    fn resolve_entry(&self, path: &Path) -> Result<String, Vec<ProjectDiagnostic>> {
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(vec![issue(
                &path.to_string_lossy(),
                "entry paths must be normalized and project-relative",
                empty_span(),
            )]);
        }
        self.resolve(path, &path.to_string_lossy(), empty_span())
    }

    fn resolve(
        &self,
        path: &Path,
        importer: &str,
        span: Span,
    ) -> Result<String, Vec<ProjectDiagnostic>> {
        if path
            .extension()
            .is_none_or(|extension| extension != "studio")
        {
            return Err(vec![issue(
                importer,
                "only local .studio component imports are supported",
                span,
            )]);
        }
        let canonical = self.root.join(path).canonicalize().map_err(|error| {
            vec![issue(
                importer,
                format!("cannot resolve {}: {error}", path.display()),
                span,
            )]
        })?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return Err(vec![issue(
                importer,
                "component source is outside project or is not a file",
                span,
            )]);
        }
        Ok(canonical
            .strip_prefix(&self.root)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .replace('\\', "/"))
    }

    fn read_bounded_source(&mut self, path: &str) -> Result<String, Vec<ProjectDiagnostic>> {
        let metadata = std::fs::metadata(self.root.join(path))
            .map_err(|error| vec![issue(path, error.to_string(), empty_span())])?;
        if metadata.len() > MAX_SOURCE_BYTES as u64 {
            return Err(vec![issue(
                path,
                "component source exceeds byte limit",
                empty_span(),
            )]);
        }
        let source = std::fs::read_to_string(self.root.join(path))
            .map_err(|error| vec![issue(path, error.to_string(), empty_span())])?;
        self.source_bytes += source.len();
        if self.source_bytes > MAX_SOURCE_BYTES {
            return Err(vec![issue(
                path,
                "component graph exceeds source byte limit",
                empty_span(),
            )]);
        }
        Ok(source)
    }

    fn load(&mut self, path: &str) -> Result<(), Vec<ProjectDiagnostic>> {
        if let Some(start) = self.loading.iter().position(|loaded| loaded == path) {
            let mut cycle = self.loading[start..].to_vec();
            cycle.push(path.to_owned());
            return Err(vec![issue(
                path,
                format!("component import cycle: {}", cycle.join(" -> ")),
                empty_span(),
            )]);
        }
        if self.modules.contains_key(path) {
            return Ok(());
        }
        if self.modules.len() + self.loading.len() >= MAX_MODULES || self.loading.len() >= MAX_DEPTH
        {
            return Err(vec![issue(
                path,
                "component graph exceeds module/depth limit",
                empty_span(),
            )]);
        }
        let source = self.read_bounded_source(path)?;
        self.loading.push(path.to_owned());
        let mut parsed = AdapterEngine::new()
            .parse_module(&source, path)
            .map_err(|errors| diagnostics(path, errors))?;
        let imports = read_imports(&source, path, &parsed)?;
        let mut resolved = BTreeMap::new();
        for import in imports {
            let relative = Path::new(path)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(&import.source);
            let imported = self.resolve(&relative, path, import.span)?;
            self.load(&imported)?;
            if resolved.insert(import.binding.clone(), imported).is_some() {
                return Err(vec![issue(
                    path,
                    format!("duplicate component import `{}`", import.binding),
                    import.span,
                )]);
            }
        }
        let timers = normalize_timers(&source, path, &mut parsed.template)?;
        let outlets = normalize_outlets(&source, &mut parsed.template);
        if outlets > 1 {
            return Err(vec![issue(
                path,
                "a component may contain only one default outlet",
                empty_span(),
            )]);
        }
        let mut names: BTreeSet<String> = resolved.keys().cloned().collect();
        names.insert("Slot".to_owned());
        let mut script = crate::validate::validate_module_with_components(&source, &parsed, &names)
            .map_err(|errors| diagnostics(path, errors))?;
        // A child snippet is compile-time structure, not a serializable guest prop.
        if outlets > 0 {
            script.props.retain(|prop| prop.name != "children");
        }
        let module = crate::lower_studio::lower_studio(&source, &parsed, &script, "component")
            .map_err(|errors| diagnostics(path, errors))?;
        let mut component = module.components.into_iter().next().ok_or_else(|| {
            vec![issue(
                path,
                "source compiled without a component",
                empty_span(),
            )]
        })?;
        validate_node_id_bounds(&component.template, path)?;
        scope_generated_branch_ids(&mut component, &parsed.template);
        validate_node_id_bounds(&component.template, path)?;
        infer_scalar_derived_types(&mut component);
        validate_timer_conditions(path, &component, &timers)?;
        self.loading.pop();
        self.modules.insert(
            path.to_owned(),
            Loaded {
                component,
                imports: resolved,
                outlets,
                timers,
            },
        );
        Ok(())
    }

    fn instantiate(
        &mut self,
        path: &str,
        prefix: &str,
        supplied: Option<&BTreeMap<String, StudioExpression>>,
        slot: Option<&[TemplateNode]>,
        depth: usize,
    ) -> Result<Composed, Vec<ProjectDiagnostic>> {
        if prefix.len() > studio_protocol::ProtocolLimits::default().max_node_id_bytes {
            return Err(vec![issue(
                path,
                "composed node id exceeds 128 bytes",
                empty_span(),
            )]);
        }
        self.instances += 1;
        if depth >= MAX_DEPTH || self.instances > MAX_INSTANCES {
            return Err(vec![issue(
                path,
                "expanded component graph exceeds instance/depth limit",
                empty_span(),
            )]);
        }
        let loaded = self
            .modules
            .get(path)
            .cloned()
            .ok_or_else(|| vec![issue(path, "component was not loaded", empty_span())])?;
        if slot.is_some() && loaded.outlets != 1 {
            return Err(vec![issue(
                path,
                "component receiving children needs exactly one default outlet",
                loaded.component.span,
            )]);
        }
        let mut component = loaded.component;
        let bindings = bind_props(path, &component, supplied)?;
        let namespace = namespace(prefix);
        scope_component(&mut component, prefix, &namespace, &bindings);
        validate_node_id_bounds(&component.template, path)?;
        if supplied.is_some() {
            component.props.clear();
        }
        let mut timers: Vec<TimerSpec> = loaded
            .timers
            .into_iter()
            .map(|timer| TimerSpec {
                id: scoped(prefix, &timer.id),
                tick_node: scoped(prefix, &timer.tick_node),
                complete_node: scoped(prefix, &timer.complete_node),
                status_node: scoped(prefix, &timer.status_node),
                interval_ms: timer.interval_ms,
                completion_delay_ms: timer.completion_delay_ms,
            })
            .collect();
        let mut locations = BTreeMap::new();
        record_locations(&component.template, path, &mut locations);
        let template = std::mem::take(&mut component.template);
        let mut context = Expansion {
            path,
            imports: &loaded.imports,
            target: &mut component,
            locations: &mut locations,
            timers: &mut timers,
            slot,
        };
        component.template = self.expand_forest(template, &mut context, false, depth)?;
        Ok(Composed {
            component,
            locations,
            timers,
        })
    }

    fn expand_forest(
        &mut self,
        nodes: Vec<TemplateNode>,
        context: &mut Expansion<'_>,
        inside_each: bool,
        depth: usize,
    ) -> Result<Vec<TemplateNode>, Vec<ProjectDiagnostic>> {
        let mut expanded = Vec::new();
        for mut node in nodes {
            self.nodes += 1;
            if self.nodes > MAX_NODES {
                return Err(vec![issue(
                    context.path,
                    "expanded component tree exceeds node limit",
                    context.target.span,
                )]);
            }
            if let TemplateNode::Component {
                id,
                kind,
                props,
                children,
                span,
                ..
            } = &node
                && kind == "Slot"
            {
                if !props.is_empty() || !children.is_empty() {
                    return Err(vec![issue(
                        context.path,
                        "default outlet cannot have props or children",
                        *span,
                    )]);
                }
                context.locations.remove(id);
                if let Some(slot) = context.slot {
                    expanded.extend_from_slice(slot);
                }
                continue;
            }
            if matches!(&node, TemplateNode::Component { kind, .. } if context.imports.contains_key(kind))
            {
                expanded.extend(self.expand_imported(node, context, inside_each, depth)?);
                continue;
            }
            self.expand_children(&mut node, context, inside_each, depth)?;
            expanded.push(node);
        }
        Ok(expanded)
    }

    fn expand_imported(
        &mut self,
        node: TemplateNode,
        context: &mut Expansion<'_>,
        inside_each: bool,
        depth: usize,
    ) -> Result<Vec<TemplateNode>, Vec<ProjectDiagnostic>> {
        let TemplateNode::Component {
            id,
            kind,
            props,
            children,
            span,
            ..
        } = node
        else {
            unreachable!("only imported component nodes reach expansion")
        };
        if context
            .target
            .handlers
            .iter()
            .any(|handler| handler.node.as_deref() == Some(id.as_str()))
        {
            return Err(vec![issue(
                context.path,
                "imported component events must be bound on native children",
                span,
            )]);
        }
        let imported = context.imports[&kind].clone();
        let provided: BTreeMap<_, _> = props
            .iter()
            .map(|binding| (binding.name.clone(), prop_expression(&binding.value)))
            .collect();
        check_prop_types(
            context.path,
            &provided,
            &self.modules[&imported].component,
            context.target,
            span,
        )?;
        let meaningful = children.iter().any(
            |child| !matches!(child, TemplateNode::Text { value, .. } if value.trim().is_empty()),
        );
        let passed_children = if meaningful {
            Some(self.expand_forest(children, context, inside_each, depth + 1)?)
        } else {
            None
        };
        let mut child = self.instantiate(
            &imported,
            &id,
            Some(&provided),
            passed_children.as_deref(),
            depth + 1,
        )?;
        if inside_each
            && (!child.component.state.is_empty()
                || !child.component.derived.is_empty()
                || !child.component.handlers.is_empty())
        {
            return Err(vec![issue(
                context.path,
                "stateful imported components in each require per-key ownership; derived slots and handlers are also unsupported",
                span,
            )]);
        }
        context.target.state.append(&mut child.component.state);
        context.target.derived.append(&mut child.component.derived);
        context
            .target
            .handlers
            .append(&mut child.component.handlers);
        context.locations.remove(&id);
        context.locations.append(&mut child.locations);
        context.timers.append(&mut child.timers);
        Ok(child.component.template)
    }

    fn expand_children(
        &mut self,
        node: &mut TemplateNode,
        context: &mut Expansion<'_>,
        inside_each: bool,
        depth: usize,
    ) -> Result<(), Vec<ProjectDiagnostic>> {
        let mut expand = |children: &mut Vec<TemplateNode>,
                          each|
         -> Result<(), Vec<ProjectDiagnostic>> {
            *children = self.expand_forest(std::mem::take(children), context, each, depth + 1)?;
            Ok(())
        };
        match node {
            TemplateNode::Component { children, .. } => expand(children, inside_each)?,
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                expand(consequent, inside_each)?;
                expand(alternate, inside_each)?;
            }
            TemplateNode::Each { body, fallback, .. } => {
                expand(body, true)?;
                expand(fallback, inside_each)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn empty_span() -> Span {
    span_for("", 0, 0)
}

fn issue(path: &str, message: impl Into<String>, span: Span) -> ProjectDiagnostic {
    ProjectDiagnostic {
        path: path.to_owned(),
        diagnostic: Diagnostic {
            code: CODE_MODULE_GRAPH,
            severity: Severity::Error,
            message: message.into(),
            span,
        },
    }
}

fn diagnostics(path: &str, diagnostics: Vec<Diagnostic>) -> Vec<ProjectDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| ProjectDiagnostic {
            path: path.to_owned(),
            diagnostic,
        })
        .collect()
}

struct Import {
    binding: String,
    source: String,
    span: Span,
}

#[derive(Debug)]
enum Token {
    Word(String),
    String(String),
    Other,
}

fn read_imports(
    source: &str,
    path: &str,
    module: &SourceModule,
) -> Result<Vec<Import>, Vec<ProjectDiagnostic>> {
    let mut imports = Vec::new();
    for script in &module.scripts {
        let tokens = tokenize(&script.content);
        let mut index = 0;
        while index < tokens.len() {
            let (token, start, _) = &tokens[index];
            if !matches!(token, Token::Word(word) if word == "import") {
                index += 1;
                continue;
            }
            let range = &tokens[index..];
            let span = span_for(
                source,
                script.content_span.start + u32::try_from(*start).unwrap_or(u32::MAX),
                script.content_span.start + u32::try_from(*start).unwrap_or(u32::MAX) + 6,
            );
            let [
                (_, _, _),
                (Token::Word(binding), _, _),
                (Token::Word(from), _, _),
                (Token::String(specifier), _, _),
                ..,
            ] = range
            else {
                return Err(vec![issue(
                    path,
                    "only local default .studio component imports are supported",
                    span,
                )]);
            };
            if from != "from"
                || !specifier.starts_with('.')
                || !specifier.ends_with(".studio")
                || !binding
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_uppercase)
            {
                return Err(vec![issue(
                    path,
                    "only local default .studio component imports are supported",
                    span,
                )]);
            }
            let converted = crate::lower::catalog_kind_name(binding);
            if matches!(binding.as_str(), "Slot" | "Timer")
                || serde_json::from_str::<studio_protocol::NodeKind>(&format!("\"{converted}\""))
                    .is_ok()
            {
                return Err(vec![issue(
                    path,
                    format!("component import `{binding}` shadows a reserved native component"),
                    span,
                )]);
            }
            imports.push(Import {
                binding: binding.clone(),
                source: specifier.clone(),
                span,
            });
            index += 4;
        }
    }
    Ok(imports)
}

// A small lexical scanner recognizes only static import declarations. It skips
// comments and quoted content, so code-looking strings do not create graph edges.
fn tokenize(text: &str) -> Vec<(Token, usize, usize)> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            byte if byte.is_ascii_whitespace() => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            quote @ (b'\'' | b'"' | b'`') => {
                index += 1;
                let content = index;
                let mut escaped = false;
                while index < bytes.len() {
                    if !escaped && bytes[index] == quote {
                        break;
                    }
                    escaped = !escaped && bytes[index] == b'\\';
                    index += 1;
                }
                tokens.push((Token::String(text[content..index].to_owned()), start, index));
                index = (index + 1).min(bytes.len());
            }
            byte if byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') => {
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'$'))
                {
                    index += 1;
                }
                tokens.push((Token::Word(text[start..index].to_owned()), start, index));
            }
            _ => {
                index += 1;
                tokens.push((Token::Other, start, index));
            }
        }
    }
    tokens
}

fn normalize_outlets(source: &str, nodes: &mut [SourceNode]) -> usize {
    let mut count = 0;
    for node in nodes {
        let replace = match node {
            SourceNode::Component { name, .. } if name == "Slot" => {
                count += 1;
                None
            }
            SourceNode::Unsupported { kind, span } if kind == "render-tag" => {
                let text = source
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default();
                let compact: String = text
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect();
                (compact == "{@renderchildren()}").then_some(*span)
            }
            _ => None,
        };
        if let Some(span) = replace {
            *node = SourceNode::Component {
                span,
                name: "Slot".to_owned(),
                attributes: Vec::new(),
                children: Vec::new(),
            };
            count += 1;
        }
        match node {
            SourceNode::Component { children, .. } | SourceNode::Element { children, .. } => {
                count += normalize_outlets(source, children);
            }
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                count += normalize_outlets(source, consequent);
                count += normalize_outlets(source, alternate);
            }
            SourceNode::Each { body, fallback, .. } => {
                count += normalize_outlets(source, body);
                count += normalize_outlets(source, fallback);
            }
            _ => {}
        }
    }
    count
}

fn bind_props(
    path: &str,
    component: &ComponentDefinition,
    supplied: Option<&BTreeMap<String, StudioExpression>>,
) -> Result<BTreeMap<String, StudioExpression>, Vec<ProjectDiagnostic>> {
    let Some(supplied) = supplied else {
        return Ok(BTreeMap::new());
    };
    for name in supplied.keys() {
        if !component.props.iter().any(|prop| &prop.name == name) {
            return Err(vec![issue(
                path,
                format!("unknown prop `{name}`"),
                component.span,
            )]);
        }
    }
    let mut bindings = BTreeMap::new();
    for prop in &component.props {
        let value = supplied
            .get(&prop.name)
            .cloned()
            .or_else(|| prop.default.clone().map(StudioExpression::Literal));
        let Some(value) = value else {
            return Err(vec![issue(
                path,
                format!("missing required prop `{}`", prop.name),
                prop.span,
            )]);
        };
        bindings.insert(prop.name.clone(), value);
    }
    Ok(bindings)
}

fn check_prop_types(
    path: &str,
    bindings: &BTreeMap<String, StudioExpression>,
    child: &ComponentDefinition,
    parent: &ComponentDefinition,
    span: Span,
) -> Result<(), Vec<ProjectDiagnostic>> {
    for prop in &child.props {
        if let Some(value) = bindings.get(&prop.name)
            && let Some(ty) = expression_type(value, parent)
            && ty != prop.ty
        {
            let mut error = issue(
                path,
                format!("prop `{}` has incompatible type", prop.name),
                span,
            );
            error.diagnostic.code = CODE_SHAPE;
            return Err(vec![error]);
        }
    }
    Ok(())
}

fn expression_type(
    expression: &StudioExpression,
    component: &ComponentDefinition,
) -> Option<StudioType> {
    match expression {
        StudioExpression::Literal(value) => Some(value.ty()),
        StudioExpression::ReadState(name) => component
            .state
            .iter()
            .find(|slot| &slot.name == name)
            .map(|slot| slot.ty.clone()),
        StudioExpression::ReadDerived(name) => component
            .derived
            .iter()
            .find(|slot| &slot.name == name)
            .map(|slot| slot.ty.clone()),
        StudioExpression::ReadProp(name) => component
            .props
            .iter()
            .find(|prop| &prop.name == name)
            .map(|prop| prop.ty.clone()),
        StudioExpression::Unary { operator, .. } => Some(if operator == "!" {
            StudioType::Boolean
        } else {
            StudioType::Number
        }),
        StudioExpression::Binary {
            operator,
            left,
            right,
        } => match operator.as_str() {
            "+" if expression_type(left, component) == Some(StudioType::String)
                || expression_type(right, component) == Some(StudioType::String) =>
            {
                Some(StudioType::String)
            }
            "+" | "-" | "*" | "/" | "%" => Some(StudioType::Number),
            _ => Some(StudioType::Boolean),
        },
        StudioExpression::Conditional { consequent, .. } => expression_type(consequent, component),
        _ => None,
    }
}

fn namespace(prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    let mut value = String::from("instance_");
    for byte in prefix.bytes() {
        let _ = write!(value, "{byte:02x}");
    }
    value.push('_');
    value
}

fn scoped(prefix: &str, value: &str) -> String {
    if prefix.is_empty() {
        value.to_owned()
    } else {
        format!("{prefix}--{value}")
    }
}

fn scope_component(
    component: &mut ComponentDefinition,
    prefix: &str,
    namespace: &str,
    bindings: &BTreeMap<String, StudioExpression>,
) {
    for slot in &mut component.state {
        slot.name = format!("{namespace}{}", slot.name);
        slot.id = slot.name.clone();
        rewrite_expression(&mut slot.initial, namespace, bindings);
    }
    for slot in &mut component.derived {
        slot.name = format!("{namespace}{}", slot.name);
        slot.id = slot.name.clone();
        rewrite_expression(&mut slot.expression, namespace, bindings);
        for dependency in &mut slot.dependencies {
            *dependency = format!("{namespace}{dependency}");
        }
    }
    for handler in &mut component.handlers {
        handler.id = scoped(prefix, &handler.id);
        handler.node = handler.node.as_ref().map(|id| scoped(prefix, id));
        rewrite_expression(&mut handler.payload, namespace, bindings);
        for mutation in &mut handler.mutations {
            mutation.slot = format!("{namespace}{}", mutation.slot);
            rewrite_expression(&mut mutation.operand, namespace, bindings);
        }
    }
    scope_nodes(&mut component.template, prefix, namespace, bindings);
}

fn rewrite_expression(
    expression: &mut StudioExpression,
    namespace: &str,
    bindings: &BTreeMap<String, StudioExpression>,
) {
    match expression {
        StudioExpression::ReadProp(name) => {
            let (root, suffix) = name
                .split_once('.')
                .map_or((name.as_str(), ""), |(root, suffix)| (root, suffix));
            if let Some(bound) = bindings.get(root) {
                let mut value = bound.clone();
                if !suffix.is_empty() {
                    match &mut value {
                        StudioExpression::ReadProp(path)
                        | StudioExpression::ReadState(path)
                        | StudioExpression::ReadDerived(path)
                        | StudioExpression::ReadLocal(path) => {
                            path.push('.');
                            path.push_str(suffix);
                        }
                        StudioExpression::Literal(value) => {
                            let mut current = value.clone();
                            for key in suffix.split('.') {
                                if let StudioValue::Record(fields) = current
                                    && let Some(next) = fields.get(key)
                                {
                                    current = next.clone();
                                } else {
                                    return;
                                }
                            }
                            *value = current;
                        }
                        _ => return,
                    }
                }
                *expression = value;
            }
        }
        StudioExpression::ReadState(name) | StudioExpression::ReadDerived(name) => {
            *name = format!("{namespace}{name}");
        }
        StudioExpression::Unary { operand, .. } => rewrite_expression(operand, namespace, bindings),
        StudioExpression::Binary { left, right, .. } => {
            rewrite_expression(left, namespace, bindings);
            rewrite_expression(right, namespace, bindings);
        }
        StudioExpression::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            rewrite_expression(condition, namespace, bindings);
            rewrite_expression(consequent, namespace, bindings);
            rewrite_expression(alternate, namespace, bindings);
        }
        StudioExpression::Array(values)
        | StudioExpression::CallApprovedFunction {
            arguments: values, ..
        } => {
            for value in values {
                rewrite_expression(value, namespace, bindings);
            }
        }
        StudioExpression::Record(fields) => {
            for (_, value) in fields {
                rewrite_expression(value, namespace, bindings);
            }
        }
        StudioExpression::Literal(_) | StudioExpression::ReadLocal(_) => {}
    }
}

fn scope_nodes(
    nodes: &mut [TemplateNode],
    prefix: &str,
    namespace: &str,
    bindings: &BTreeMap<String, StudioExpression>,
) {
    for node in nodes {
        match node {
            TemplateNode::Component {
                id,
                props,
                children,
                explicit_key,
                ..
            } => {
                *id = scoped(prefix, id);
                if let Some(key) = explicit_key {
                    rewrite_expression(key, namespace, bindings);
                }
                for prop in props {
                    if let PropValue::Expression(value) = &mut prop.value {
                        rewrite_expression(value, namespace, bindings);
                    }
                }
                scope_nodes(children, prefix, namespace, bindings);
            }
            TemplateNode::Text { id, .. } => *id = scoped(prefix, id),
            TemplateNode::Interpolation { id, expression, .. } => {
                *id = scoped(prefix, id);
                rewrite_expression(expression, namespace, bindings);
            }
            TemplateNode::If {
                id,
                condition,
                consequent,
                alternate,
                ..
            } => {
                *id = scoped(prefix, id);
                rewrite_expression(condition, namespace, bindings);
                scope_nodes(consequent, prefix, namespace, bindings);
                scope_nodes(alternate, prefix, namespace, bindings);
            }
            TemplateNode::Each {
                id,
                collection,
                key,
                body,
                fallback,
                ..
            } => {
                *id = scoped(prefix, id);
                rewrite_expression(collection, namespace, bindings);
                rewrite_expression(key, namespace, bindings);
                scope_nodes(body, prefix, namespace, bindings);
                scope_nodes(fallback, prefix, namespace, bindings);
            }
        }
    }
}

fn record_locations(
    nodes: &[TemplateNode],
    path: &str,
    locations: &mut BTreeMap<String, ProjectSourceLocation>,
) {
    for node in nodes {
        let (id, span) = match node {
            TemplateNode::Component { id, span, .. }
            | TemplateNode::Text { id, span, .. }
            | TemplateNode::Interpolation { id, span, .. }
            | TemplateNode::If { id, span, .. }
            | TemplateNode::Each { id, span, .. } => (id, span),
        };
        locations.insert(
            id.clone(),
            ProjectSourceLocation {
                path: path.to_owned(),
                span: *span,
            },
        );
        match node {
            TemplateNode::Component { children, .. } => record_locations(children, path, locations),
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                record_locations(consequent, path, locations);
                record_locations(alternate, path, locations);
            }
            TemplateNode::Each { body, fallback, .. } => {
                record_locations(body, path, locations);
                record_locations(fallback, path, locations);
            }
            _ => {}
        }
    }
}

fn prop_expression(value: &PropValue) -> StudioExpression {
    match value {
        PropValue::Literal(value) => StudioExpression::Literal(value.clone()),
        PropValue::Expression(value) => value.clone(),
    }
}

fn collect_content(nodes: &[TemplateNode]) -> Vec<ContentBinding> {
    let mut content = Vec::new();
    for node in nodes {
        match node {
            TemplateNode::Component {
                id,
                kind,
                props,
                children,
                ..
            } => {
                if let Some(prop) = crate::lower_studio::content_prop(kind) {
                    let explicit = props.iter().find(|binding| binding.name == prop);
                    let runs: Vec<StudioExpression> = children
                        .iter()
                        .filter_map(|child| match child {
                            TemplateNode::Text { value, .. } if !value.trim().is_empty() => Some(
                                StudioExpression::Literal(StudioValue::String(value.clone())),
                            ),
                            TemplateNode::Interpolation { expression, .. } => {
                                Some(expression.clone())
                            }
                            _ => None,
                        })
                        .collect();
                    let mut runs = runs.into_iter();
                    let expression = explicit
                        .map(|binding| prop_expression(&binding.value))
                        .or_else(|| {
                            runs.next().map(|first| {
                                runs.fold(first, |left, right| StudioExpression::Binary {
                                    operator: "+".to_owned(),
                                    left: Box::new(left),
                                    right: Box::new(right),
                                })
                            })
                        });
                    if let Some(expression) = expression {
                        content.push(ContentBinding {
                            node_id: id.clone(),
                            prop: prop.to_owned(),
                            expression,
                        });
                    }
                } else {
                    content.extend(collect_content(children));
                }
            }
            TemplateNode::Text { id, value, .. } if !value.trim().is_empty() => {
                content.push(ContentBinding {
                    node_id: id.clone(),
                    prop: "text".to_owned(),
                    expression: StudioExpression::Literal(StudioValue::String(value.clone())),
                });
            }
            TemplateNode::Interpolation { id, expression, .. } => content.push(ContentBinding {
                node_id: id.clone(),
                prop: "text".to_owned(),
                expression: expression.clone(),
            }),
            _ => {}
        }
    }
    content
}

fn collect_dependencies(expression: &StudioExpression, dependencies: &mut BTreeSet<String>) {
    match expression {
        StudioExpression::ReadProp(name)
        | StudioExpression::ReadState(name)
        | StudioExpression::ReadDerived(name) => {
            dependencies.insert(name.split('.').next().unwrap_or_default().to_owned());
        }
        StudioExpression::Unary { operand, .. } => collect_dependencies(operand, dependencies),
        StudioExpression::Binary { left, right, .. } => {
            collect_dependencies(left, dependencies);
            collect_dependencies(right, dependencies);
        }
        StudioExpression::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            collect_dependencies(condition, dependencies);
            collect_dependencies(consequent, dependencies);
            collect_dependencies(alternate, dependencies);
        }
        StudioExpression::Array(values)
        | StudioExpression::CallApprovedFunction {
            arguments: values, ..
        } => {
            for value in values {
                collect_dependencies(value, dependencies);
            }
        }
        StudioExpression::Record(fields) => {
            for (_, value) in fields {
                collect_dependencies(value, dependencies);
            }
        }
        StudioExpression::Literal(_) | StudioExpression::ReadLocal(_) => {}
    }
}

fn validate_composed(
    component: &ComponentDefinition,
    locations: &BTreeMap<String, ProjectSourceLocation>,
    entry: &str,
) -> Result<(), Vec<ProjectDiagnostic>> {
    let mut ids = BTreeSet::new();
    let mut nodes: Vec<_> = component.template.iter().collect();
    while let Some(node) = nodes.pop() {
        let (id, span) = match node {
            TemplateNode::Component { id, span, .. }
            | TemplateNode::Text { id, span, .. }
            | TemplateNode::Interpolation { id, span, .. }
            | TemplateNode::If { id, span, .. }
            | TemplateNode::Each { id, span, .. } => (id, span),
        };
        if !ids.insert(id.as_str()) {
            let path = locations
                .get(id)
                .map_or(entry, |location| location.path.as_str());
            return Err(vec![issue(
                path,
                format!("duplicate composed node `{id}`"),
                *span,
            )]);
        }
        match node {
            TemplateNode::Component { children, .. } => nodes.extend(children),
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                nodes.extend(consequent);
                nodes.extend(alternate);
            }
            TemplateNode::Each { body, fallback, .. } => {
                nodes.extend(body);
                nodes.extend(fallback);
            }
            _ => {}
        }
    }
    let mut bindings = BTreeSet::new();
    for (name, span) in component
        .props
        .iter()
        .map(|prop| (&prop.name, prop.span))
        .chain(component.state.iter().map(|slot| (&slot.name, slot.span)))
        .chain(component.derived.iter().map(|slot| (&slot.name, slot.span)))
    {
        if !bindings.insert(name.as_str()) {
            return Err(vec![issue(
                entry,
                format!("duplicate composed binding `{name}`"),
                span,
            )]);
        }
    }
    Ok(())
}

fn merge_public_props(
    path: &str,
    props: &mut Vec<crate::ir::PropDefinition>,
    incoming: Vec<crate::ir::PropDefinition>,
) -> Result<(), Vec<ProjectDiagnostic>> {
    for prop in incoming {
        if let Some(existing) = props.iter_mut().find(|existing| existing.name == prop.name) {
            if existing.ty != prop.ty {
                return Err(vec![issue(
                    path,
                    format!("shared layout prop `{}` has incompatible type", prop.name),
                    prop.span,
                )]);
            }
            if existing.required || prop.required {
                existing.required = true;
                existing.default = None;
            } else if existing.default != prop.default {
                return Err(vec![issue(
                    path,
                    format!(
                        "shared layout prop `{}` has conflicting defaults",
                        prop.name
                    ),
                    prop.span,
                )]);
            }
        } else {
            props.push(prop);
        }
    }
    Ok(())
}

fn normalize_timers(
    source: &str,
    path: &str,
    nodes: &mut [SourceNode],
) -> Result<Vec<TimerSpec>, Vec<ProjectDiagnostic>> {
    let mut timers = Vec::new();
    for node in nodes {
        if let SourceNode::Component {
            name,
            attributes,
            children,
            span,
        } = node
            && name == "Timer"
        {
            if children.iter().any(
                |child| !matches!(child, SourceNode::Text { value, .. } if value.trim().is_empty()),
            ) {
                return Err(vec![issue(
                    path,
                    "Timer cannot contain children",
                    span_for(source, span.start, span.end),
                )]);
            }
            let (timer, normalized) = normalize_timer(source, path, attributes, *span)?;
            timers.push(timer);
            *node = normalized;
            continue;
        }
        match node {
            SourceNode::Component { children, .. } | SourceNode::Element { children, .. } => {
                timers.extend(normalize_timers(source, path, children)?);
            }
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                timers.extend(normalize_timers(source, path, consequent)?);
                timers.extend(normalize_timers(source, path, alternate)?);
            }
            SourceNode::Each { body, fallback, .. } => {
                timers.extend(normalize_timers(source, path, body)?);
                timers.extend(normalize_timers(source, path, fallback)?);
            }
            _ => {}
        }
    }
    Ok(timers)
}

struct TimerInputs {
    id: String,
    interval_ms: u64,
    completion_delay_ms: u64,
    until: SourceExpression,
    tick: SourceExpression,
    complete: SourceExpression,
}

fn timer_handler(
    attributes: &[SourceAttribute],
    name: &str,
    path: &str,
    location: Span,
) -> Result<SourceExpression, Vec<ProjectDiagnostic>> {
    timer_expression(attributes, name)
        .filter(|expression| {
            expression
                .json
                .get("type")
                .and_then(serde_json::Value::as_str)
                == Some("Identifier")
        })
        .ok_or_else(|| {
            vec![issue(
                path,
                format!("Timer {name} needs a named handler reference"),
                location,
            )]
        })
}

fn timer_inputs(
    source: &str,
    path: &str,
    attributes: &[SourceAttribute],
    span: SourceSpan,
) -> Result<TimerInputs, Vec<ProjectDiagnostic>> {
    let location = span_for(source, span.start, span.end);
    let allowed = [
        "id",
        "interval_ms",
        "completion_delay_ms",
        "until",
        "ontick",
        "oncomplete",
    ];
    for attribute in attributes {
        if !allowed.contains(&attribute.name.as_str()) {
            return Err(vec![issue(
                path,
                format!("unknown Timer prop `{}`", attribute.name),
                location,
            )]);
        }
    }
    let id = attributes
        .iter()
        .find(|attribute| attribute.name == "id")
        .and_then(|attribute| {
            if let SourceAttributeValue::Sequence(parts) = &attribute.value
                && let [SourceAttributePart::Text(id)] = parts.as_slice()
            {
                Some(id.clone())
            } else {
                None
            }
        })
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 96
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .ok_or_else(|| {
            vec![issue(
                path,
                "Timer needs an explicit stable ASCII id of at most 96 bytes",
                location,
            )]
        })?;
    let interval_ms = timer_integer(attributes, "interval_ms", None, 1000, 60_000)
        .map_err(|message| vec![issue(path, message, location)])?;
    let completion_delay_ms = timer_integer(attributes, "completion_delay_ms", Some(0), 0, 60_000)
        .map_err(|message| vec![issue(path, message, location)])?;
    let until = timer_expression(attributes, "until").ok_or_else(|| {
        vec![issue(
            path,
            "Timer until needs a boolean expression",
            location,
        )]
    })?;
    let tick = timer_handler(attributes, "ontick", path, location)?;
    let complete = timer_handler(attributes, "oncomplete", path, location)?;
    Ok(TimerInputs {
        id,
        interval_ms,
        completion_delay_ms,
        until,
        tick,
        complete,
    })
}

fn normalize_timer(
    source: &str,
    path: &str,
    attributes: &[SourceAttribute],
    span: SourceSpan,
) -> Result<(TimerSpec, SourceNode), Vec<ProjectDiagnostic>> {
    let TimerInputs {
        id,
        interval_ms,
        completion_delay_ms,
        until,
        tick,
        complete,
    } = timer_inputs(source, path, attributes, span)?;
    let timer = TimerSpec {
        tick_node: format!("{id}--tick"),
        complete_node: format!("{id}--complete"),
        status_node: format!("{id}--status"),
        id,
        interval_ms,
        completion_delay_ms,
    };
    let conditional = SourceExpression {
        span: until.span,
        text: format!("{} ? 'complete' : 'running'", until.text),
        json: serde_json::json!({
            "type": "ConditionalExpression", "test": until.json,
            "consequent": { "type": "Literal", "value": "complete", "raw": "\"complete\"" },
            "alternate": { "type": "Literal", "value": "running", "raw": "\"running\"" },
        }),
    };
    let tick_node = timer_button(&timer.tick_node, tick, span);
    let complete_node = timer_button(&timer.complete_node, complete, span);
    let status = SourceNode::Component {
        span,
        name: "Text".to_owned(),
        attributes: vec![
            text_attribute("id", &timer.status_node, span),
            SourceAttribute {
                span,
                name: "text".to_owned(),
                value: SourceAttributeValue::Expression(conditional),
            },
        ],
        children: Vec::new(),
    };
    let normalized = SourceNode::Component {
        span,
        name: "Box".to_owned(),
        attributes: vec![
            text_attribute("id", &timer.id, span),
            SourceAttribute {
                span,
                name: "visible".to_owned(),
                value: SourceAttributeValue::Expression(SourceExpression {
                    span,
                    text: "false".to_owned(),
                    json: serde_json::json!({ "type": "Literal", "value": false, "raw": "false" }),
                }),
            },
        ],
        children: vec![tick_node, complete_node, status],
    };
    Ok((timer, normalized))
}

fn timer_expression(attributes: &[SourceAttribute], name: &str) -> Option<SourceExpression> {
    attributes.iter().find_map(|attribute| {
        if attribute.name == name
            && let SourceAttributeValue::Expression(expression) = &attribute.value
        {
            Some(expression.clone())
        } else {
            None
        }
    })
}

fn timer_integer(
    attributes: &[SourceAttribute],
    name: &str,
    default: Option<u64>,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let Some(attribute) = attributes.iter().find(|attribute| attribute.name == name) else {
        return default.ok_or_else(|| format!("Timer {name} is required"));
    };
    let SourceAttributeValue::Expression(expression) = &attribute.value else {
        return Err(format!("Timer {name} must be a static integer"));
    };
    let number = expression
        .json
        .get("value")
        .and_then(serde_json::Value::as_f64)
        .filter(|number| number.is_finite() && number.fract() == 0.0)
        .ok_or_else(|| format!("Timer {name} must be a static integer"))?;
    let value = format!("{number:.0}")
        .parse::<u64>()
        .map_err(|_| format!("Timer {name} must be between {minimum} and {maximum}"))?;
    if value < minimum || value > maximum {
        return Err(format!(
            "Timer {name} must be between {minimum} and {maximum}"
        ));
    }
    Ok(value)
}

fn text_attribute(name: &str, value: &str, span: SourceSpan) -> SourceAttribute {
    SourceAttribute {
        span,
        name: name.to_owned(),
        value: SourceAttributeValue::Sequence(vec![SourceAttributePart::Text(value.to_owned())]),
    }
}

fn timer_button(id: &str, handler: SourceExpression, span: SourceSpan) -> SourceNode {
    SourceNode::Component {
        span,
        name: "Button".to_owned(),
        attributes: vec![
            text_attribute("id", id, span),
            text_attribute("label", "Timer event", span),
            SourceAttribute {
                span,
                name: "onclick".to_owned(),
                value: SourceAttributeValue::Expression(handler),
            },
        ],
        children: Vec::new(),
    }
}

fn validate_timer_conditions(
    path: &str,
    component: &ComponentDefinition,
    timers: &[TimerSpec],
) -> Result<(), Vec<ProjectDiagnostic>> {
    for timer in timers {
        let mut nodes: Vec<_> = component.template.iter().collect();
        while let Some(node) = nodes.pop() {
            match node {
                TemplateNode::Component {
                    id,
                    props,
                    children,
                    span,
                    ..
                } => {
                    if id == &timer.status_node {
                        let condition = props.iter().find_map(|binding| match &binding.value {
                            PropValue::Expression(StudioExpression::Conditional {
                                condition,
                                ..
                            }) => Some(condition.as_ref()),
                            _ => None,
                        });
                        if !condition.is_some_and(|condition| {
                            is_boolean_condition(condition, component, &mut BTreeSet::new())
                        }) {
                            return Err(vec![issue(
                                path,
                                "Timer until expression must have boolean type",
                                *span,
                            )]);
                        }
                    }
                    nodes.extend(children);
                }
                TemplateNode::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    nodes.extend(consequent);
                    nodes.extend(alternate);
                }
                TemplateNode::Each { body, fallback, .. } => {
                    nodes.extend(body);
                    nodes.extend(fallback);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn is_boolean_condition(
    expression: &StudioExpression,
    component: &ComponentDefinition,
    visiting: &mut BTreeSet<String>,
) -> bool {
    match expression {
        StudioExpression::Literal(StudioValue::Boolean(_)) => true,
        StudioExpression::ReadState(name) => component
            .state
            .iter()
            .any(|slot| &slot.name == name && slot.ty == StudioType::Boolean),
        StudioExpression::ReadProp(name) => component
            .props
            .iter()
            .any(|prop| &prop.name == name && prop.ty == StudioType::Boolean),
        StudioExpression::ReadDerived(name) => {
            if !visiting.insert(name.clone()) {
                return false;
            }
            let boolean = component
                .derived
                .iter()
                .find(|slot| &slot.name == name)
                .is_some_and(|slot| is_boolean_condition(&slot.expression, component, visiting));
            visiting.remove(name);
            boolean
        }
        StudioExpression::Unary { operator, .. } => operator == "!",
        StudioExpression::Binary {
            operator,
            left,
            right,
        } => match operator.as_str() {
            "==" | "===" | "!=" | "!==" | "<" | "<=" | ">" | ">=" => true,
            "&&" | "||" => {
                is_boolean_condition(left, component, visiting)
                    && is_boolean_condition(right, component, visiting)
            }
            _ => false,
        },
        StudioExpression::Conditional {
            consequent,
            alternate,
            ..
        } => {
            is_boolean_condition(consequent, component, visiting)
                && is_boolean_condition(alternate, component, visiting)
        }
        _ => false,
    }
}

fn validate_timer_targets(
    component: &ComponentDefinition,
    timers: &[TimerSpec],
    path: &str,
) -> Result<(), Vec<ProjectDiagnostic>> {
    if timers.len() > 16 {
        return Err(vec![issue(
            path,
            "a page may declare at most 16 timers",
            component.span,
        )]);
    }
    let mut static_ids = BTreeSet::new();
    let mut nodes: Vec<_> = component.template.iter().map(|node| (node, true)).collect();
    while let Some((node, is_static)) = nodes.pop() {
        match node {
            TemplateNode::Component { id, children, .. } => {
                if is_static {
                    static_ids.insert(id.as_str());
                }
                nodes.extend(children.iter().map(|child| (child, is_static)));
            }
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => nodes.extend(
                consequent
                    .iter()
                    .chain(alternate)
                    .map(|child| (child, false)),
            ),
            TemplateNode::Each { body, fallback, .. } => {
                nodes.extend(body.iter().chain(fallback).map(|child| (child, false)));
            }
            _ => {}
        }
    }
    for timer in timers {
        for id in [
            &timer.id,
            &timer.tick_node,
            &timer.complete_node,
            &timer.status_node,
        ] {
            if id.len() > studio_protocol::ProtocolLimits::default().max_node_id_bytes
                || !static_ids.contains(id.as_str())
            {
                return Err(vec![issue(
                    path,
                    "Timer targets must be bounded and declared in a static component tree",
                    component.span,
                )]);
            }
        }
    }
    Ok(())
}

fn validate_node_id_bounds(
    nodes: &[TemplateNode],
    path: &str,
) -> Result<(), Vec<ProjectDiagnostic>> {
    let maximum = studio_protocol::ProtocolLimits::default().max_node_id_bytes;
    let mut pending: Vec<_> = nodes.iter().collect();
    while let Some(node) = pending.pop() {
        let (id, span) = match node {
            TemplateNode::Component { id, span, .. }
            | TemplateNode::Text { id, span, .. }
            | TemplateNode::Interpolation { id, span, .. }
            | TemplateNode::If { id, span, .. }
            | TemplateNode::Each { id, span, .. } => (id, span),
        };
        if id.is_empty() || id.len() > maximum || id.chars().any(char::is_control) {
            return Err(vec![issue(
                path,
                "node id must contain 1 to 128 bytes without controls",
                *span,
            )]);
        }
        match node {
            TemplateNode::Component { children, .. } => pending.extend(children),
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                pending.extend(consequent);
                pending.extend(alternate);
            }
            TemplateNode::Each { body, fallback, .. } => {
                pending.extend(body);
                pending.extend(fallback);
            }
            _ => {}
        }
    }
    Ok(())
}

fn infer_scalar_derived_types(component: &mut ComponentDefinition) {
    for index in 0..component.derived.len() {
        if let Some(ty) = expression_type(&component.derived[index].expression, component) {
            component.derived[index].ty = ty;
        }
    }
}

fn scope_generated_branch_ids(component: &mut ComponentDefinition, source: &[SourceNode]) {
    let mut explicit = BTreeSet::new();
    collect_explicit_source_ids(source, &mut explicit);
    scope_branch_forest(
        &mut component.template,
        "",
        &explicit,
        &mut component.handlers,
    );
}

fn collect_explicit_source_ids(nodes: &[SourceNode], explicit: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            SourceNode::Component {
                attributes,
                children,
                ..
            }
            | SourceNode::Element {
                attributes,
                children,
                ..
            } => {
                for attribute in attributes {
                    if attribute.name == "id"
                        && let SourceAttributeValue::Sequence(parts) = &attribute.value
                        && let [SourceAttributePart::Text(id)] = parts.as_slice()
                    {
                        explicit.insert(id.clone());
                    }
                }
                collect_explicit_source_ids(children, explicit);
            }
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                collect_explicit_source_ids(consequent, explicit);
                collect_explicit_source_ids(alternate, explicit);
            }
            SourceNode::Each { body, fallback, .. } => {
                collect_explicit_source_ids(body, explicit);
                collect_explicit_source_ids(fallback, explicit);
            }
            _ => {}
        }
    }
}

fn scope_branch_forest(
    nodes: &mut [TemplateNode],
    branch: &str,
    explicit: &BTreeSet<String>,
    handlers: &mut [crate::ir::EventHandler],
) {
    for node in nodes {
        let (id, span) = match node {
            TemplateNode::Component { id, span, .. }
            | TemplateNode::Text { id, span, .. }
            | TemplateNode::Interpolation { id, span, .. }
            | TemplateNode::If { id, span, .. }
            | TemplateNode::Each { id, span, .. } => (id, *span),
        };
        if !branch.is_empty() && !explicit.contains(id.as_str()) {
            let scoped = format!("{id}--branch-{branch}");
            for handler in &mut *handlers {
                if handler.node.as_deref() == Some(id.as_str())
                    && (span.start.offset..=span.end.offset).contains(&handler.span.start.offset)
                {
                    handler.node = Some(scoped.clone());
                    handler.id = format!("{scoped}#{}", handler.event);
                }
            }
            *id = scoped;
        }
        match node {
            TemplateNode::Component { children, .. } => {
                scope_branch_forest(children, branch, explicit, handlers);
            }
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                scope_branch_forest(consequent, &format!("{branch}t"), explicit, handlers);
                scope_branch_forest(alternate, &format!("{branch}f"), explicit, handlers);
            }
            TemplateNode::Each { body, fallback, .. } => {
                scope_branch_forest(body, &format!("{branch}b"), explicit, handlers);
                scope_branch_forest(fallback, &format!("{branch}e"), explicit, handlers);
            }
            _ => {}
        }
    }
}
