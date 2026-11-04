//! Namespaced page guests with host-authorized navigation and retained page state.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;
use studio_navigation::project::{ProjectRoute, RoutingManifest};
use studio_protocol::{GuestMessage, NavigationCommand, NodeKind};

use super::{
    ContentTy, EmittedStudio, StudioEmitError, asc_string_literal, content_ty, emit_bool_expr,
    emit_failed, emit_mutation, emit_num_expr, emit_str_expr, slot_ty, stateful_prelude,
    ts_default, ts_type,
};
use crate::ir::{
    ComponentDefinition, EventHandler, PropValue, StudioExpression, StudioModule, TemplateNode,
};
use crate::types::{StudioType, StudioValue};

/// Emit all declared pages into one guest, retaining each page's reactive globals.
///
/// Navigation requests remain protocol-v1 commands. Only an accepted host navigation
/// result changes the active namespace and causes a fresh projection of that page.
///
/// # Errors
/// Rejects invalid/missing page declarations or expressions the Wasm backend cannot lower.
pub fn emit_routed_studio(
    module: &StudioModule,
    routing: &RoutingManifest,
) -> Result<EmittedStudio, StudioEmitError> {
    routing.validate().map_err(|message| StudioEmitError {
        code: crate::CODE_MODULE_GRAPH,
        message,
    })?;
    let initial = routing.initial_route().map_err(|message| StudioEmitError {
        code: crate::CODE_MODULE_GRAPH,
        message,
    })?;
    if module.components.len() != routing.routes.len() {
        return Err(emit_failed(
            "route declarations that do not match compiled pages",
        ));
    }
    let initial_index = routing
        .routes
        .iter()
        .position(|entry| entry.path == initial)
        .ok_or_else(|| emit_failed("a missing startup page"))?;
    let mut source = String::from("// Generated routed Studio project. DO NOT EDIT.\n");
    source.push_str("@external(\"studio_host\", \"emit\")\ndeclare function hostEmit(pointer: i32, length: i32): i32;\n");
    source.push_str("function emit(message: string): i32 { const bytes = String.UTF8.encode(message, false); return hostEmit(changetype<i32>(bytes), bytes.byteLength); }\n");
    source.push_str(
        "function studioRetained(): void {}\nlet studioTableEntry: () => void = studioRetained;\n",
    );
    source.push_str(JSON_STRING_READER);
    let _ = writeln!(source, "let studioActivePage: i32 = {initial_index};");
    let _ = writeln!(
        source,
        "let studioCurrentRoute: string = {};",
        asc_string_literal(&initial)
    );
    let mut initial_mount = None;
    for (index, route) in routing.routes.iter().enumerate() {
        let (page_source, mount) =
            emit_page_source(module, route, index, &initial, index == initial_index)?;
        source.push_str(&page_source);
        if mount.is_some() {
            initial_mount = mount;
        }
    }
    source.push_str(&project_dispatch_source(routing));
    Ok(EmittedStudio {
        assembly_source: source,
        mount_payload: initial_mount.ok_or_else(|| emit_failed("an absent initial mount"))?,
    })
}

fn emit_page_source(
    module: &StudioModule,
    route: &ProjectRoute,
    index: usize,
    initial: &str,
    is_initial: bool,
) -> Result<(String, Option<String>), StudioEmitError> {
    let mut source = String::new();
    let mut initial_mount = None;
    let mut page = module
        .components
        .iter()
        .find(|component| component.id == route.id)
        .cloned()
        .ok_or_else(|| emit_failed("a route with no compiled page"))?;
    let parameters: Vec<_> = route
        .path
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .collect();
    for prop in &mut page.props {
        if parameters.contains(&prop.name.as_str())
            && prop.ty == StudioType::String
            && prop.default.is_none()
        {
            prop.default = Some(StudioValue::String(String::new()));
            prop.required = false;
        }
    }
    let component = &page;
    let mut single = module.clone();
    single.components = vec![component.clone()];
    let (mut mount, scope) = crate::eval::mount_tree(&single, component, &BTreeMap::new())
        .map_err(|error| StudioEmitError {
            code: error.code,
            message: error.message,
        })?;
    initial.clone_into(&mut mount.route);
    if is_initial {
        initial_mount = Some(
            serde_json::to_string(&GuestMessage::Mount(mount.clone()))
                .map_err(|_| emit_failed("mount serialization"))?,
        );
    }
    let _ = writeln!(source, "namespace StudioPage{index} {{");
    let mut prelude = stateful_prelude(component)?;
    for slot in &component.state {
        if slot.ty == StudioType::Number {
            prelude = prelude.replace(
                &format!("let state_{} = ", slot.name),
                &format!("let state_{}: f64 = ", slot.name),
            );
        }
    }
    source.push_str(&prelude);
    source.push_str(&navigation_snapshot_source(component));
    let tree = tree_expression(component, &scope)?;
    source.push_str("export function mount(route: string): i32 {\n");
    source.push_str(&parameter_assignments(component, &route.path));
    source.push_str("  recomputeStudioDerived();\n  studioPatchSequence = 0;\n  return emit(");
    let _ = writeln!(
        source,
        "{} + studioQuote(route) + {} + {tree} + {});\n}}",
        asc_string_literal("{\"type\":\"mount\",\"payload\":{\"protocol_version\":1,\"route\":"),
        asc_string_literal(",\"root\":"),
        asc_string_literal("}}")
    );
    source.push_str("export function event(event: string): i32 {\n");
    for handler in &component.handlers {
        source.push_str(&emit_handler_source(component, &scope, &tree, handler)?);
    }
    source.push_str("  return 0;\n}\n}\n");
    Ok((source, initial_mount))
}

fn emit_handler_source(
    component: &ComponentDefinition,
    scope: &crate::eval::EvalScope,
    tree: &str,
    handler: &EventHandler,
) -> Result<String, StudioEmitError> {
    let mut source = String::new();
    let Some(node) = handler.node.as_deref() else {
        return Ok(String::new());
    };
    let _ = writeln!(
        source,
        "  if (studioReadString(event, \"node_id\") == {} && studioReadString(event, \"event\") == {}) {{",
        asc_string_literal(node),
        asc_string_literal(&handler.event)
    );
    if handler.emit == "navigate" {
        source.push_str("    studioSaveNavigationState();\n");
    }
    for mutation in &handler.mutations {
        let _ = writeln!(source, "    {}", emit_mutation(component, mutation)?);
    }
    source.push_str("    recomputeStudioDerived();\n");
    if handler.emit.is_empty() {
        source.push_str("    studioPatchSequence += 1;\n    return emit(");
        let _ = writeln!(
            source,
            "{} + studioPatchSequence.toString() + {} + {tree} + {});",
            asc_string_literal("{\"type\":\"patch\",\"payload\":{\"sequence\":"),
            asc_string_literal(&format!(
                ",\"operations\":[{{\"op\":\"replace_node\",\"node_id\":{},\"node\":",
                serde_json::to_string(&format!("{}#root", component.id))
                    .map_err(|_| emit_failed("root identities"))?
            )),
            asc_string_literal("}]}}")
        );
    } else {
        // Validate the closed command shape with the evaluator at compile time.
        // Payload values are emitted dynamically below, after all state mutations.
        if handler.emit == "navigate" {
            validate_navigation_payload(&handler.payload)?;
        } else {
            crate::eval::dispatch_event(
                component,
                scope,
                node,
                &handler.event,
                &component.handlers,
            )
            .map_err(|error| StudioEmitError {
                code: error.code,
                message: error.message,
            })?;
        }
        let envelope = if handler.emit == "navigate" {
            "{\"type\":\"navigate\",\"payload\":".to_owned()
        } else {
            format!(
                "{{\"type\":\"event\",\"name\":{},\"payload\":",
                serde_json::to_string(&handler.emit).map_err(|_| emit_failed("event names"))?
            )
        };
        let _ = writeln!(
            source,
            "    return emit({} + {} + {});",
            asc_string_literal(&envelope),
            json_expression(component, &handler.payload)?,
            asc_string_literal("}")
        );
    }
    source.push_str("  }\n");
    Ok(source)
}

fn project_dispatch_source(routing: &RoutingManifest) -> String {
    let mut source = String::new();
    source.push_str(&resolver_source(routing));
    source.push_str("function studioMount(): i32 {\n");
    for index in 0..routing.routes.len() {
        let _ = writeln!(
            source,
            "  if (studioActivePage == {index}) return StudioPage{index}.mount(studioCurrentRoute);"
        );
    }
    source.push_str("  return 0;\n}\n");
    source.push_str("export function studio_alloc(length: i32): i32 { if (length < 0 || length > 65536) return 0; return heap.alloc(length > 0 ? length : 8) as i32; }\nexport function studio_dealloc(_pointer: i32, _length: i32): void {}\nexport function studio_init(_pointer: i32, _length: i32): i32 { studioTableEntry(); return studioMount(); }\n");
    source.push_str("export function studio_event(pointer: i32, length: i32): i32 {\n  studioTableEntry = studioRetained;\n  const event = String.UTF8.decodeUnsafe(pointer, length, false);\n  if (studioReadString(event, \"type\") == \"navigation\") {\n    const accepted = event.includes(\"\\\"accepted\\\":true\");\n");
    for index in 0..routing.routes.len() {
        let _ = writeln!(
            source,
            "    if (studioActivePage == {index}) StudioPage{index}.finishNavigation(accepted);"
        );
    }
    source.push_str("    if (!accepted) return 0;\n    const route = studioReadString(event, \"route\");\n    const selected = studioResolveRoute(route);\n    if (selected < 0) return 0;\n    studioActivePage = selected; studioCurrentRoute = route; return studioMount();\n  }\n  if (studioReadString(event, \"type\") != \"ui\") return 0;\n");
    for index in 0..routing.routes.len() {
        let _ = writeln!(
            source,
            "  if (studioActivePage == {index}) return StudioPage{index}.event(event);"
        );
    }
    source.push_str("  return 0;\n}\n");
    source
}

fn navigation_snapshot_source(component: &ComponentDefinition) -> String {
    let mut source = String::from("let studioNavigationPending: bool = false;\n");
    for slot in &component.state {
        let ty = slot_ty(Some(&slot.ty));
        if ty == ContentTy::Other {
            continue;
        }
        let _ = writeln!(
            source,
            "let saved_{}: {} = {};",
            slot.name,
            ts_type(ty),
            ts_default(ty)
        );
    }
    source.push_str(
        "function studioSaveNavigationState(): void {\n  studioNavigationPending = true;\n",
    );
    for slot in &component.state {
        if slot_ty(Some(&slot.ty)) != ContentTy::Other {
            let _ = writeln!(source, "  saved_{} = state_{};", slot.name, slot.name);
        }
    }
    source.push_str("}\nexport function finishNavigation(accepted: bool): void {\n  if (!studioNavigationPending) return;\n  if (!accepted) {\n");
    for slot in &component.state {
        if slot_ty(Some(&slot.ty)) != ContentTy::Other {
            let _ = writeln!(source, "    state_{} = saved_{};", slot.name, slot.name);
        }
    }
    source.push_str("    recomputeStudioDerived();\n  }\n  studioNavigationPending = false;\n}\n");
    source
}

fn validate_navigation_payload(payload: &StudioExpression) -> Result<(), StudioEmitError> {
    let StudioExpression::Record(entries) = payload else {
        return Err(emit_failed("non-record navigation payloads"));
    };
    let mut object = serde_json::Map::new();
    for (name, value) in entries {
        let json = match value {
            StudioExpression::Literal(literal) => crate::eval::studio_value_json(literal),
            _ if name == "route" => Value::String("/".to_owned()),
            _ => return Err(emit_failed("non-literal navigation operation names")),
        };
        if object.insert(name.clone(), json).is_some() {
            return Err(emit_failed("duplicate navigation payload keys"));
        }
    }
    let command: NavigationCommand = serde_json::from_value(Value::Object(object))
        .map_err(|_| emit_failed("invalid navigation command fields"))?;
    studio_protocol::navigation::validate_navigation_command(&command)
        .map_err(|_| emit_failed("invalid navigation paths"))
}

fn parameter_assignments(component: &ComponentDefinition, pattern: &str) -> String {
    let mut source = String::new();
    for (index, segment) in pattern.trim_start_matches('/').split('/').enumerate() {
        if let Some(name) = segment.strip_prefix(':')
            && component
                .props
                .iter()
                .any(|prop| prop.name == name && prop.ty == StudioType::String)
        {
            let _ = writeln!(source, "  prop_{name} = route.split(\"/\")[{}];", index + 1);
        }
    }
    source
}

fn resolver_source(routing: &RoutingManifest) -> String {
    let mut entries: Vec<_> = routing.routes.iter().enumerate().collect();
    entries.sort_by(|(_, left), (_, right)| {
        specificity(&right.path)
            .cmp(&specificity(&left.path))
            .then(left.path.cmp(&right.path))
    });
    let mut source = String::from(
        "function studioResolveRoute(route: string): i32 {\n  const parts = route == \"/\" ? new Array<string>() : route.substring(1).split(\"/\");\n",
    );
    for (index, entry) in entries {
        let segments: Vec<_> = entry
            .path
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let wildcard = segments.last() == Some(&"*");
        let fixed = segments.len() - usize::from(wildcard);
        let mut checks = vec![format!(
            "parts.length {} {fixed}",
            if wildcard { ">=" } else { "==" }
        )];
        for (position, segment) in segments.iter().take(fixed).enumerate() {
            if !segment.starts_with(':') {
                checks.push(format!(
                    "parts[{position}] == {}",
                    asc_string_literal(segment)
                ));
            }
        }
        let _ = writeln!(source, "  if ({}) return {index};", checks.join(" && "));
    }
    source.push_str("  return -1;\n}\n");
    source
}

fn specificity(path: &str) -> Vec<u8> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part == "*" {
                0
            } else if part.starts_with(':') {
                2
            } else {
                3
            }
        })
        .chain(std::iter::once(1))
        .collect()
}

fn tree_expression(
    component: &ComponentDefinition,
    scope: &crate::eval::EvalScope,
) -> Result<String, StudioEmitError> {
    let children = forest_expression(component, &component.template, scope)?;
    Ok(node_expression(
        &format!("{}#root", component.id),
        "box",
        &asc_string_literal("{}"),
        &children,
    ))
}

fn node_expression(id: &str, kind: &str, props: &str, children: &str) -> String {
    format!(
        "{} + {props} + {} + {children} + {}",
        asc_string_literal(&format!(
            "{{\"id\":{},\"kind\":{},\"props\":",
            serde_json::to_string(id).unwrap_or_default(),
            serde_json::to_string(kind).unwrap_or_default()
        )),
        asc_string_literal(",\"children\":"),
        asc_string_literal("}")
    )
}

fn forest_expression(
    component: &ComponentDefinition,
    nodes: &[TemplateNode],
    scope: &crate::eval::EvalScope,
) -> Result<String, StudioEmitError> {
    let mut rendered = Vec::new();
    for node in nodes {
        if let Some(expression) = template_expression(component, node, scope)? {
            rendered.push(expression);
        }
    }
    let mut result = asc_string_literal("[");
    for (index, expression) in rendered.iter().enumerate() {
        if index > 0 {
            result.push_str(" + \",\"");
        }
        let _ = write!(result, " + ({expression})");
    }
    result.push_str(" + \"]\"");
    Ok(result)
}

fn template_expression(
    component: &ComponentDefinition,
    node: &TemplateNode,
    scope: &crate::eval::EvalScope,
) -> Result<Option<String>, StudioEmitError> {
    let expression = match node {
        TemplateNode::Component {
            id,
            kind,
            props,
            children,
            ..
        } => component_template_expression(component, id, kind, props, children, scope)?,
        TemplateNode::Text { id, value, .. } => {
            if value.trim().is_empty() {
                return Ok(None);
            }
            node_expression(
                id,
                "text",
                &asc_string_literal(&serde_json::json!({"text":value}).to_string()),
                &asc_string_literal("[]"),
            )
        }
        TemplateNode::Interpolation { id, expression, .. } => node_expression(
            id,
            "text",
            &format!(
                "{} + studioQuote({}) + {}",
                asc_string_literal("{\"text\":"),
                emit_str_expr(component, None, expression)?,
                asc_string_literal("}")
            ),
            &asc_string_literal("[]"),
        ),
        TemplateNode::If {
            id,
            condition,
            consequent,
            alternate,
            ..
        } => {
            let children = format!(
                "({} ? ({}) : ({}))",
                emit_bool_expr(component, None, condition)?,
                forest_expression(component, consequent, scope)?,
                forest_expression(component, alternate, scope)?
            );
            node_expression(
                &format!("{id}#branch"),
                "box",
                &asc_string_literal("{}"),
                &children,
            )
        }
        TemplateNode::Each { .. } => static_each_expression(component, node, scope)?,
    };
    Ok(Some(expression))
}

fn component_template_expression(
    component: &ComponentDefinition,
    id: &str,
    kind: &str,
    bindings: &[crate::ir::PropBinding],
    children: &[TemplateNode],
    scope: &crate::eval::EvalScope,
) -> Result<String, StudioEmitError> {
    let catalog = crate::lower::catalog_kind_name(kind);
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    for binding in bindings {
        values.insert(
            binding.name.clone(),
            match &binding.value {
                PropValue::Literal(value) => literal_json(value)?,
                PropValue::Expression(value) => json_expression(component, value)?,
            },
        );
    }
    let is_leaf = matches!(catalog.as_str(), "text" | "button");
    if is_leaf {
        if let Some(binding) = component
            .content
            .iter()
            .find(|binding| binding.node_id == id)
        {
            values.insert(
                binding.prop.clone(),
                format!(
                    "studioQuote({})",
                    emit_str_expr(component, None, &binding.expression)?
                ),
            );
        } else if let Some(content) = folded_children_expression(component, children)? {
            values.insert(
                if catalog == "text" { "text" } else { "label" }.to_owned(),
                format!("studioQuote({content})"),
            );
        }
    }
    let mut props = asc_string_literal("{");
    for (index, (key, value)) in values.iter().enumerate() {
        let prefix = format!(
            "{}{}:",
            if index > 0 { "," } else { "" },
            serde_json::to_string(key).map_err(|_| emit_failed("prop keys"))?
        );
        let _ = write!(props, " + {} + ({value})", asc_string_literal(&prefix));
    }
    props.push_str(" + \"}\"");
    let mut child_json = if is_leaf {
        asc_string_literal("[]")
    } else {
        forest_expression(component, children, scope)?
    };
    let kind: NodeKind = serde_json::from_value(Value::String(catalog.clone()))
        .map_err(|_| emit_failed("unknown catalog kinds"))?;
    let count = children
        .iter()
        .filter(|node| !matches!(node,TemplateNode::Text{value,..} if value.trim().is_empty()))
        .count();
    if !is_leaf
        && studio_protocol::child_cardinality(kind) == studio_protocol::ChildCardinality::AtMostOne
        && count > 1
    {
        child_json = format!(
            "{} + ({}) + {}",
            asc_string_literal("["),
            node_expression(
                &format!("{id}#wrap"),
                "column",
                &asc_string_literal("{}"),
                &child_json
            ),
            asc_string_literal("]")
        );
    }
    Ok(node_expression(id, &catalog, &props, &child_json))
}

fn static_each_expression(
    component: &ComponentDefinition,
    node: &TemplateNode,
    scope: &crate::eval::EvalScope,
) -> Result<String, StudioEmitError> {
    // Static collections retain the existing evaluator's exact keyed identities.
    // Dynamic list codegen has its own closed backend contract; never freeze live state.
    let encoded = serde_json::to_string(node).map_err(|_| emit_failed("each templates"))?;
    if encoded.contains("ReadState") || encoded.contains("ReadDerived") {
        return Err(emit_failed(
            "dynamic repeated content in routed page remounts",
        ));
    }
    let mut projected = component.clone();
    projected.template = vec![node.clone()];
    let mut module = StudioModule {
        version: crate::ir::STUDIO_IR_VERSION,
        id: component.id.clone(),
        imports: Vec::new(),
        exports: vec![component.name.clone()],
        components: vec![projected.clone()],
        dependencies: Vec::new(),
    };
    module.components[0].props.clone_from(&component.props);
    let (mount, _) = crate::eval::mount_tree(&module, &module.components[0], &scope.props)
        .map_err(|error| StudioEmitError {
            code: error.code,
            message: error.message,
        })?;
    let child = mount
        .root
        .children
        .first()
        .ok_or_else(|| emit_failed("an empty repeated subtree"))?;
    Ok(asc_string_literal(
        &serde_json::to_string(child).map_err(|_| emit_failed("each serialization"))?,
    ))
}

fn folded_children_expression(
    component: &ComponentDefinition,
    children: &[TemplateNode],
) -> Result<Option<String>, StudioEmitError> {
    let mut pieces = Vec::new();
    for child in children {
        match child {
            TemplateNode::Text { value, .. } if value.trim().is_empty() => {}
            TemplateNode::Text { value, .. } => pieces.push(asc_string_literal(value)),
            TemplateNode::Interpolation { expression, .. } => {
                pieces.push(emit_str_expr(component, None, expression)?);
            }
            _ => {
                return Err(emit_failed(
                    "nested structural content inside a text/button leaf",
                ));
            }
        }
    }
    Ok((!pieces.is_empty()).then(|| format!("({})", pieces.join(" + "))))
}

fn literal_json(value: &StudioValue) -> Result<String, StudioEmitError> {
    Ok(asc_string_literal(
        &serde_json::to_string(&crate::eval::studio_value_json(value))
            .map_err(|_| emit_failed("literal JSON"))?,
    ))
}

fn json_expression(
    component: &ComponentDefinition,
    expression: &StudioExpression,
) -> Result<String, StudioEmitError> {
    match expression {
        StudioExpression::Conditional {
            condition,
            consequent,
            alternate,
        } => Ok(format!(
            "({} ? ({}) : ({}))",
            emit_bool_expr(component, None, condition)?,
            json_expression(component, consequent)?,
            json_expression(component, alternate)?,
        )),
        StudioExpression::Literal(value) => literal_json(value),
        StudioExpression::Record(values) => {
            let mut sorted: Vec<_> = values.iter().collect();
            sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut source = asc_string_literal("{");
            for (index, (key, value)) in sorted.iter().enumerate() {
                let prefix = format!(
                    "{}{}:",
                    if index > 0 { "," } else { "" },
                    serde_json::to_string(key).map_err(|_| emit_failed("record keys"))?
                );
                let _ = write!(
                    source,
                    " + {} + ({})",
                    asc_string_literal(&prefix),
                    json_expression(component, value)?
                );
            }
            source.push_str(" + \"}\"");
            Ok(source)
        }
        StudioExpression::Array(values) => {
            let mut source = asc_string_literal("[");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    source.push_str(" + \",\"");
                }
                let _ = write!(source, " + ({})", json_expression(component, value)?);
            }
            source.push_str(" + \"]\"");
            Ok(source)
        }
        _ => match content_ty(component, None, expression) {
            ContentTy::String => Ok(format!(
                "studioQuote({})",
                emit_str_expr(component, None, expression)?
            )),
            ContentTy::Number => Ok(format!(
                "studioNumberToString({})",
                emit_num_expr(component, None, expression)?
            )),
            ContentTy::Boolean => Ok(format!(
                "({} ? \"true\" : \"false\")",
                emit_bool_expr(component, None, expression)?
            )),
            ContentTy::Other => Err(emit_failed("untyped dynamic JSON")),
        },
    }
}

const JSON_STRING_READER: &str = r#"
function studioQuote(value: string): string {
  let out = "\"";
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code == 34) out += "\\\"";
    else if (code == 92) out += "\\\\";
    else if (code == 10) out += "\\n";
    else if (code == 13) out += "\\r";
    else if (code == 9) out += "\\t";
    else if (code < 32) {
      const hex = "0123456789abcdef";
      out += "\\u00" + hex.charAt(code >> 4) + hex.charAt(code & 15);
    }
    else out += String.fromCharCode(code);
  }
  return out + "\"";
}
function studioReadString(document: string, key: string): string {
  const marker = "\"" + key + "\"";
  let cursor = document.indexOf(marker);
  if (cursor < 0) return "";
  cursor += marker.length;
  while (cursor < document.length && document.charCodeAt(cursor) <= 32) cursor++;
  if (document.charCodeAt(cursor++) != 58) return "";
  while (cursor < document.length && document.charCodeAt(cursor) <= 32) cursor++;
  if (document.charCodeAt(cursor++) != 34) return "";
  let out = "";
  while (cursor < document.length) {
    let code = document.charCodeAt(cursor++);
    if (code == 34) return out;
    if (code == 92) {
      code = document.charCodeAt(cursor++);
      if (code == 110) out += "\n";
      else if (code == 114) out += "\r";
      else if (code == 116) out += "\t";
      else out += String.fromCharCode(code);
    } else out += String.fromCharCode(code);
  }
  return "";
}
"#;
