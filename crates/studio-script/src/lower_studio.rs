//! AST-to-IR lowering: validated adapter output becomes typed Studio IR.
//!
//! The lowerer classifies every expression against the closed expression set;
//! anything outside it fails with a `STUDIO320`/`STUDIO321` diagnostic. Node
//! identities are positional paths (`Component/0/2`), so identical sources
//! always produce byte-identical modules.
use crate::ir::{
    ComponentDefinition, DerivedSlot, EventHandler, PropBinding, PropDefinition, PropValue,
    STUDIO_IR_VERSION, StateSlot, StudioExpression, StudioModule, TemplateNode,
};
use crate::rsvelte_adapter::{
    SourceAttribute, SourceAttributeValue, SourceExpression, SourceModule, SourceNode, span_for,
};
use crate::types::{StudioType, StudioValue};
use crate::validate::{ValidatedScript, trigger_for_event};
use crate::{
    CODE_NO_LOWERING_RULE, CODE_STUDIO_UNKNOWN, CODE_UNKNOWN_CALL, Diagnostic, Severity, Span,
};

/// Approved pure functions callable from template expressions.
const APPROVED_CALLS: &[&str] = &["formatMoney"];

/// Lower one validated module into typed Studio IR.
///
/// `name` is the component name (normally the filename stem).
///
/// # Errors
///
/// Returns diagnostics when an expression or binding has no lowering rule.
pub fn lower_studio(
    source: &str,
    module: &SourceModule,
    script: &ValidatedScript,
    name: &str,
) -> Result<StudioModule, Vec<Diagnostic>> {
    let mut worker = Lowerer {
        source,
        script,
        component: name.to_owned(),
        handlers: Vec::new(),
        content: Vec::new(),
        diagnostics: Vec::new(),
    };
    let template = worker.lower_forest(&module.template, &[], "root");
    let handlers = std::mem::take(&mut worker.handlers);
    let mut content = std::mem::take(&mut worker.content);
    worker.record_forest_content(&template, &mut content);
    if !worker.diagnostics.is_empty() {
        return Err(worker.diagnostics);
    }
    Ok(StudioModule {
        version: STUDIO_IR_VERSION,
        id: name.to_owned(),
        imports: Vec::new(),
        exports: vec![name.to_owned()],
        components: vec![ComponentDefinition {
            id: name.to_owned(),
            name: name.to_owned(),
            props: script
                .props
                .iter()
                .map(|prop| PropDefinition {
                    name: prop.name.clone(),
                    ty: prop
                        .default
                        .as_ref()
                        .map_or(StudioType::String, StudioValue::ty),
                    required: prop.required,
                    default: prop.default.clone(),
                    span: prop.span,
                })
                .collect(),
            state: script
                .state
                .iter()
                .map(|slot| StateSlot {
                    id: format!("{name}#{}", slot.name),
                    name: slot.name.clone(),
                    ty: slot.ty.clone(),
                    initial: StudioExpression::Literal(slot.value.clone()),
                    span: slot.span,
                })
                .collect(),
            derived: script
                .derived
                .iter()
                .map(|slot| {
                    let expression = worker.classify_text(&slot.expr_text, &[], slot.span);
                    DerivedSlot {
                        id: format!("{name}#{}", slot.name),
                        name: slot.name.clone(),
                        ty: StudioType::Number,
                        expression,
                        dependencies: slot_dependencies(&slot.expr_text, script),
                        span: slot.span,
                    }
                })
                .collect(),
            handlers,
            content,
            template,
            span: span_for(source, 0, u32::try_from(source.len()).unwrap_or(u32::MAX)),
        }],
        dependencies: Vec::new(),
    })
}

/// Names referenced by a derivation expression, for reevaluation ordering.
fn slot_dependencies(expr_text: &str, script: &ValidatedScript) -> Vec<String> {
    let mut dependencies = Vec::new();
    for candidate in script
        .props
        .iter()
        .map(|prop| prop.name.clone())
        .chain(script.state.iter().map(|slot| slot.name.clone()))
        .chain(script.derived.iter().map(|slot| slot.name.clone()))
    {
        if contains_word(expr_text, &candidate) && !dependencies.contains(&candidate) {
            dependencies.push(candidate);
        }
    }
    dependencies.sort();
    dependencies
}

pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut search = 0usize;
    while let Some(found) = haystack[search..].find(needle) {
        let absolute = search + found;
        let before = haystack[..absolute].chars().next_back();
        let after = haystack[absolute + needle.len()..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|character| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
            })
        };
        if boundary(before) && boundary(after) {
            return true;
        }
        search = absolute + needle.len();
    }
    false
}

/// One template event binding: trigger, emit name, payload, and mutations.
type NodeHandler = (String, String, StudioExpression, Vec<crate::ir::Mutation>);

struct Lowerer<'a> {
    source: &'a str,
    script: &'a ValidatedScript,
    component: String,
    handlers: Vec<EventHandler>,
    content: Vec<crate::ir::ContentBinding>,
    diagnostics: Vec<Diagnostic>,
}

impl Lowerer<'_> {
    fn span(&self, span: crate::rsvelte_adapter::SourceSpan) -> Span {
        span_for(self.source, span.start, span.end)
    }

    fn error(&mut self, code: &'static str, message: String, span: Span) {
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            message,
            span,
        });
    }

    /// Record content bindings for one lowered forest.
    ///
    /// `Text`/`Button` components fold their text runs into one content
    /// expression; raw text runs and top-level interpolations record
    /// directly. `If`/`Each` subtrees are skipped: their content is
    /// structural, and patches never address nodes that may not exist.
    /// Recorded bindings drive both the evaluator's patches and the
    /// `AssemblyScript` backend, so the legs agree by construction.
    fn record_forest_content(
        &mut self,
        nodes: &[TemplateNode],
        content: &mut Vec<crate::ir::ContentBinding>,
    ) {
        for node in nodes {
            match node {
                TemplateNode::Component {
                    id,
                    kind,
                    explicit_key: _,
                    props,
                    children,
                    span,
                } => {
                    // Folded `Text`/`Button` content is represented by the
                    // component binding alone: recursing would double-record
                    // the runs under positional ids that never mount.
                    if !self.fold_component_content(id, kind, props, children, *span, content) {
                        self.record_forest_content(children, content);
                    }
                }
                TemplateNode::Text { id, value, .. } => {
                    if !value.trim().is_empty() {
                        content.push(crate::ir::ContentBinding {
                            node_id: id.clone(),
                            prop: "text".to_owned(),
                            expression: StudioExpression::Literal(StudioValue::String(
                                value.clone(),
                            )),
                        });
                    }
                }
                TemplateNode::Interpolation { id, expression, .. } => {
                    content.push(crate::ir::ContentBinding {
                        node_id: id.clone(),
                        prop: "text".to_owned(),
                        expression: expression.clone(),
                    });
                }
                TemplateNode::If { .. } | TemplateNode::Each { .. } => {}
            }
        }
    }

    /// Lower one event attribute into a node handler binding.
    fn lower_event_handler(
        &mut self,
        attribute: &SourceAttribute,
        trigger: &str,
        locals: &[String],
        span: Span,
    ) -> Option<NodeHandler> {
        let SourceAttributeValue::Expression(expression) = &attribute.value else {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("event `{}` needs a handler reference", attribute.name),
                span,
            );
            return None;
        };
        let Some(function) = function_reference(expression) else {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("event `{}` needs a plain handler reference", attribute.name),
                span,
            );
            return None;
        };
        let Some(definition) = self
            .script
            .functions
            .iter()
            .find(|definition| definition.name == *function)
            .map(|definition| {
                (
                    definition.emissions.clone(),
                    definition.mutations.clone(),
                    definition.name.clone(),
                )
            })
        else {
            self.error(
                CODE_UNKNOWN_CALL,
                format!("handler `{function}` is not defined"),
                span,
            );
            return None;
        };
        if definition.0.len() > 1 {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("handler `{}` must emit at most once", definition.2),
                span,
            );
            return None;
        }
        let (emit, payload) = match definition.0.first() {
            Some(emission) => (
                emission.event.clone(),
                self.classify_record(&emission.payload_text, locals, span),
            ),
            None => (
                String::new(),
                StudioExpression::Literal(StudioValue::String(String::new())),
            ),
        };
        let mut mutations = Vec::new();
        for mutation in &definition.1 {
            let operand = if mutation.operand_text.trim().is_empty() {
                StudioExpression::Literal(StudioValue::Number(crate::types::NumberLiteral {
                    text: "0".to_owned(),
                    value: 0.0,
                }))
            } else {
                self.classify_text(&mutation.operand_text, locals, mutation.span)
            };
            mutations.push(crate::ir::Mutation {
                slot: mutation.slot.clone(),
                op: match mutation.op {
                    crate::validate::MutationOp::Assign => crate::ir::MutationOp::Assign,
                    crate::validate::MutationOp::Add => crate::ir::MutationOp::Add,
                    crate::validate::MutationOp::Sub => crate::ir::MutationOp::Sub,
                    crate::validate::MutationOp::Inc => crate::ir::MutationOp::Inc,
                    crate::validate::MutationOp::Dec => crate::ir::MutationOp::Dec,
                    crate::validate::MutationOp::Push => crate::ir::MutationOp::Push,
                    crate::validate::MutationOp::Pop => crate::ir::MutationOp::Pop,
                    crate::validate::MutationOp::Remove => crate::ir::MutationOp::Remove,
                    crate::validate::MutationOp::Clear => crate::ir::MutationOp::Clear,
                },
                operand,
                span: mutation.span,
            });
        }
        Some((trigger.to_owned(), emit, payload, mutations))
    }

    fn lower_forest(
        &mut self,
        nodes: &[SourceNode],
        locals: &[String],
        path: &str,
    ) -> Vec<TemplateNode> {
        nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let child_path = if path == "root" {
                    index.to_string()
                } else {
                    format!("{path}.{index}")
                };
                self.lower_node(node, locals, &child_path)
            })
            .collect()
    }

    fn node_id(&self, path: &str) -> String {
        format!("{}#{path}", self.component)
    }

    fn lower_component(
        &mut self,
        name: &str,
        attributes: &[SourceAttribute],
        children: &[SourceNode],
        span: crate::rsvelte_adapter::SourceSpan,
        locals: &[String],
        path: &str,
    ) -> TemplateNode {
        let (props, node_handlers) = self.lower_attributes(name, attributes, locals);
        // Explicit `id` attributes are the author-controlled identity and win
        // over positional paths, so adding a sibling never renumbers retained
        // nodes. The validator rejects duplicates.
        let id = explicit_node_id(attributes).unwrap_or_else(|| self.node_id(path));
        let handler_span = self.span(span);
        self.handlers.extend(
            node_handlers
                .into_iter()
                .map(|(event, emit, payload, mutations)| EventHandler {
                    id: format!("{id}#{event}"),
                    node: Some(id.clone()),
                    event,
                    emit,
                    payload,
                    mutations,
                    span: handler_span,
                }),
        );
        let lowered_children = self.lower_forest(children, locals, path);
        TemplateNode::Component {
            id,
            kind: name.to_owned(),
            explicit_key: None,
            props,
            children: lowered_children,
            span: self.span(span),
        }
    }

    fn lower_node(&mut self, node: &SourceNode, locals: &[String], path: &str) -> TemplateNode {
        match node {
            SourceNode::Component {
                name,
                attributes,
                children,
                span,
            } => self.lower_component(name, attributes, children, *span, locals, path),
            SourceNode::Element { span, .. } => {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    "HTML elements cannot be lowered".to_owned(),
                    self.span(*span),
                );
                TemplateNode::Text {
                    id: self.node_id(path),
                    value: String::new(),
                    span: self.span(*span),
                }
            }
            SourceNode::Text { span, value } => TemplateNode::Text {
                id: self.node_id(path),
                value: value.clone(),
                span: self.span(*span),
            },
            SourceNode::Interpolation(expression) => {
                let lowered =
                    self.classify_json(&expression.json, locals, self.span(expression.span));
                TemplateNode::Interpolation {
                    id: self.node_id(path),
                    expression: lowered,
                    span: self.span(expression.span),
                }
            }
            SourceNode::If {
                test,
                consequent,
                alternate,
                span,
            } => TemplateNode::If {
                id: self.node_id(path),
                condition: self.classify_json(&test.json, locals, self.span(test.span)),
                consequent: self.lower_forest(consequent, locals, path),
                alternate: self.lower_forest(alternate, locals, path),
                span: self.span(*span),
            },
            SourceNode::Each {
                collection,
                item,
                index,
                key,
                body,
                fallback,
                span,
            } => {
                let mut scoped: Vec<String> = locals.to_vec();
                scoped.push(item.clone());
                if let Some(index) = index {
                    scoped.push(index.clone());
                }
                let key_expression = if let Some(key) = key {
                    self.classify_json(&key.json, &scoped, self.span(key.span))
                } else {
                    self.error(
                        CODE_STUDIO_UNKNOWN,
                        "keyed iteration requires a key expression".to_owned(),
                        self.span(*span),
                    );
                    StudioExpression::Literal(StudioValue::String(String::new()))
                };
                TemplateNode::Each {
                    id: self.node_id(path),
                    collection: self.classify_json(
                        &collection.json,
                        locals,
                        self.span(collection.span),
                    ),
                    item: item.clone(),
                    index: index.clone(),
                    key: key_expression,
                    body: self.lower_forest(body, &scoped, path),
                    fallback: self.lower_forest(fallback, locals, path),
                    span: self.span(*span),
                }
            }
            SourceNode::Unsupported { span, kind } => {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!("`{kind}` cannot be lowered"),
                    self.span(*span),
                );
                TemplateNode::Text {
                    id: self.node_id(path),
                    value: String::new(),
                    span: self.span(*span),
                }
            }
        }
    }

    fn lower_attributes(
        &mut self,
        _component: &str,
        attributes: &[SourceAttribute],
        locals: &[String],
    ) -> (Vec<PropBinding>, Vec<NodeHandler>) {
        let mut props = Vec::new();
        let mut handlers = Vec::new();
        for attribute in attributes {
            let span = self.span(attribute.span);
            // `id` is node identity, resolved separately by the caller —
            // never a mount property (the catalog rejects it).
            if attribute.name == "id" {
                continue;
            }
            if let Some(trigger) = trigger_for_event(&attribute.name) {
                if let Some(handler) = self.lower_event_handler(attribute, trigger, locals, span) {
                    handlers.push(handler);
                }
                continue;
            }
            match &attribute.value {
                SourceAttributeValue::True => props.push(PropBinding {
                    name: attribute.name.clone(),
                    value: PropValue::Literal(StudioValue::Boolean(true)),
                    span,
                }),
                SourceAttributeValue::Sequence(parts) => {
                    if parts.len() == 1
                        && let crate::rsvelte_adapter::SourceAttributePart::Text(text) = &parts[0]
                    {
                        props.push(PropBinding {
                            name: attribute.name.clone(),
                            value: PropValue::Literal(StudioValue::String(text.clone())),
                            span,
                        });
                        continue;
                    }
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!("mixed content for `{}` is not supported", attribute.name),
                        span,
                    );
                }
                SourceAttributeValue::Expression(expression) => {
                    let lowered =
                        self.classify_json(&expression.json, locals, self.span(expression.span));
                    let value = match lowered {
                        StudioExpression::Literal(literal) => PropValue::Literal(literal),
                        dynamic => PropValue::Expression(dynamic),
                    };
                    props.push(PropBinding {
                        name: attribute.name.clone(),
                        value,
                        span,
                    });
                }
            }
        }
        (props, handlers)
    }

    fn classify_text(&mut self, text: &str, locals: &[String], span: Span) -> StudioExpression {
        match crate::expression::parse_text_expression(text.trim()) {
            Ok(parsed) => self.lower_text_expr(&parsed, locals, span),
            Err(message) => {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("expression `{}` {message}", text.trim()),
                    span,
                );
                StudioExpression::Literal(StudioValue::String(String::new()))
            }
        }
    }

    fn lower_text_expr(
        &mut self,
        parsed: &crate::expression::TextExpr,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        use crate::expression::TextExpr;
        match parsed {
            TextExpr::Literal(text) => {
                if let Some(literal) = crate::validate::parse_literal(text) {
                    StudioExpression::Literal(literal)
                } else {
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!("literal `{text}` is not portable"),
                        span,
                    );
                    StudioExpression::Literal(StudioValue::String(String::new()))
                }
            }
            TextExpr::Path(path) => {
                if is_path_reference(path, self.script, locals) {
                    read_binding(self.script, path)
                } else {
                    self.error(
                        CODE_UNKNOWN_CALL,
                        format!("`{path}` is not a known binding"),
                        span,
                    );
                    StudioExpression::Literal(StudioValue::String(String::new()))
                }
            }
            TextExpr::Unary { operator, operand } => {
                if !matches!(operator.as_str(), "!" | "-" | "+") {
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!("unary operator `{operator}` is not portable"),
                        span,
                    );
                    return StudioExpression::Literal(StudioValue::String(String::new()));
                }
                StudioExpression::Unary {
                    operator: operator.clone(),
                    operand: Box::new(self.lower_text_expr(operand, locals, span)),
                }
            }
            TextExpr::Binary {
                operator,
                left,
                right,
            } => StudioExpression::Binary {
                operator: operator.clone(),
                left: Box::new(self.lower_text_expr(left, locals, span)),
                right: Box::new(self.lower_text_expr(right, locals, span)),
            },
            TextExpr::Conditional {
                condition,
                consequent,
                alternate,
            } => StudioExpression::Conditional {
                condition: Box::new(self.lower_text_expr(condition, locals, span)),
                consequent: Box::new(self.lower_text_expr(consequent, locals, span)),
                alternate: Box::new(self.lower_text_expr(alternate, locals, span)),
            },
            TextExpr::Array(elements) => StudioExpression::Array(
                elements
                    .iter()
                    .map(|element| self.lower_text_expr(element, locals, span))
                    .collect(),
            ),
            TextExpr::Record(entries) => StudioExpression::Record(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), self.lower_text_expr(value, locals, span)))
                    .collect(),
            ),
            TextExpr::Call {
                function,
                arguments,
            } => {
                if !APPROVED_CALLS.contains(&function.as_str()) {
                    self.error(
                        CODE_UNKNOWN_CALL,
                        format!("call to `{function}` is not an approved function"),
                        span,
                    );
                    return StudioExpression::Literal(StudioValue::String(String::new()));
                }
                StudioExpression::CallApprovedFunction {
                    function: function.clone(),
                    arguments: arguments
                        .iter()
                        .map(|argument| self.lower_text_expr(argument, locals, span))
                        .collect(),
                }
            }
        }
    }

    fn classify_record(&mut self, text: &str, locals: &[String], span: Span) -> StudioExpression {
        // Payloads are records or bare values; shorthand (`{ quantity }`) is
        // expanded by the reader before lowering.
        match self.classify_text(text, locals, span) {
            StudioExpression::Literal(StudioValue::String(empty)) if empty.is_empty() => {
                StudioExpression::Record(Vec::new())
            }
            lowered => lowered,
        }
    }

    fn classify_json(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let kind = json.get("type").and_then(|kind| kind.as_str());
        // Reuse the text-carrying path by synthesizing a text-free expression.
        let text = json
            .get("raw")
            .and_then(|raw| raw.as_str())
            .unwrap_or_default()
            .to_owned();
        match kind {
            Some("Literal") => self.lower_literal(json, &text, span),
            Some("Identifier") => self.lower_identifier(json, locals, span),
            Some("MemberExpression") => {
                let path = node_text(self.source, json).and_then(member_path_from_text);
                match path {
                    Some(path) if is_path_reference(&path, self.script, locals) => {
                        read_binding(self.script, &path)
                    }
                    _ => {
                        self.error(
                            CODE_NO_LOWERING_RULE,
                            "member path is not portable".to_owned(),
                            span,
                        );
                        StudioExpression::Literal(StudioValue::String(String::new()))
                    }
                }
            }
            Some("CallExpression") => self.lower_call(json, locals, span),
            Some("UnaryExpression") => self.lower_unary(json, locals, span),
            Some("BinaryExpression" | "LogicalExpression") => self.lower_binary(json, locals, span),
            Some("ConditionalExpression") => self.lower_conditional(json, locals, span),
            Some("ArrayExpression" | "ObjectExpression") => {
                self.lower_collection(json, locals, span)
            }
            _ => {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    "expression has no lowering rule".to_owned(),
                    span,
                );
                StudioExpression::Literal(StudioValue::String(String::new()))
            }
        }
    }

    fn lower_literal(
        &mut self,
        json: &serde_json::Value,
        text: &str,
        span: Span,
    ) -> StudioExpression {
        let Some(literal) = literal_from_json(json, text) else {
            self.error(
                CODE_NO_LOWERING_RULE,
                "literal is not portable".to_owned(),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        };
        StudioExpression::Literal(literal)
    }

    fn lower_identifier(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let name = json
            .get("name")
            .and_then(|name| name.as_str())
            .unwrap_or_default();
        if is_path_reference(name, self.script, locals) {
            read_binding(self.script, name)
        } else {
            self.error(
                CODE_UNKNOWN_CALL,
                format!("`{name}` is not a known binding"),
                span,
            );
            StudioExpression::Literal(StudioValue::String(String::new()))
        }
    }

    fn lower_call(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let callee = json
            .pointer("/callee/name")
            .and_then(|name| name.as_str())
            .unwrap_or_default();
        if !APPROVED_CALLS.contains(&callee) {
            self.error(
                CODE_UNKNOWN_CALL,
                format!("call to `{callee}` is not an approved function"),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        }
        let mut arguments = Vec::new();
        if let Some(list) = json.get("arguments").and_then(|list| list.as_array()) {
            for argument in list {
                arguments.push(self.classify_json(argument, locals, span));
            }
        }
        StudioExpression::CallApprovedFunction {
            function: callee.to_owned(),
            arguments,
        }
    }

    fn lower_unary(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let operator = json
            .get("operator")
            .and_then(|operator| operator.as_str())
            .unwrap_or_default();
        if !matches!(operator, "!" | "-" | "+") {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("unary operator `{operator}` is not portable"),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        }
        let Some(argument) = json.get("argument") else {
            self.error(
                CODE_NO_LOWERING_RULE,
                "unary expression is missing its operand".to_owned(),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        };
        StudioExpression::Unary {
            operator: operator.to_owned(),
            operand: Box::new(self.classify_json(argument, locals, span)),
        }
    }

    fn lower_binary(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let (Some(left), Some(right)) = (json.get("left"), json.get("right")) else {
            self.error(
                CODE_NO_LOWERING_RULE,
                "binary expression is missing operands".to_owned(),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        };
        let operator = json
            .get("operator")
            .and_then(|operator| operator.as_str())
            .unwrap_or("?")
            .to_owned();
        StudioExpression::Binary {
            operator,
            left: Box::new(self.classify_json(left, locals, span)),
            right: Box::new(self.classify_json(right, locals, span)),
        }
    }

    fn lower_conditional(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        let (Some(test), Some(consequent), Some(alternate)) = (
            json.get("test"),
            json.get("consequent"),
            json.get("alternate"),
        ) else {
            self.error(
                CODE_NO_LOWERING_RULE,
                "conditional expression is incomplete".to_owned(),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        };
        StudioExpression::Conditional {
            condition: Box::new(self.classify_json(test, locals, span)),
            consequent: Box::new(self.classify_json(consequent, locals, span)),
            alternate: Box::new(self.classify_json(alternate, locals, span)),
        }
    }

    fn lower_collection(
        &mut self,
        json: &serde_json::Value,
        locals: &[String],
        span: Span,
    ) -> StudioExpression {
        if json.get("type").and_then(|kind| kind.as_str()) == Some("ArrayExpression") {
            let mut elements = Vec::new();
            if let Some(list) = json.get("elements").and_then(|list| list.as_array()) {
                for element in list {
                    if element.is_null() {
                        continue;
                    }
                    elements.push(self.classify_json(element, locals, span));
                }
            }
            return StudioExpression::Array(elements);
        }
        let Some(entries) = object_entries(json) else {
            self.error(
                CODE_NO_LOWERING_RULE,
                "object literal is not portable".to_owned(),
                span,
            );
            return StudioExpression::Literal(StudioValue::String(String::new()));
        };
        let mut record = Vec::new();
        for (key, value) in entries {
            record.push((key, self.classify_json(&value, locals, span)));
        }
        StudioExpression::Record(record)
    }
}

/// Read an explicit static `id` attribute value, if the node declares one.
fn explicit_node_id(attributes: &[SourceAttribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| {
        if attribute.name != "id" {
            return None;
        }
        match &attribute.value {
            SourceAttributeValue::Sequence(parts) if parts.len() == 1 => match &parts[0] {
                crate::rsvelte_adapter::SourceAttributePart::Text(text) => Some(text.clone()),
                crate::rsvelte_adapter::SourceAttributePart::Interpolation(_) => None,
            },
            SourceAttributeValue::Sequence(_)
            | SourceAttributeValue::True
            | SourceAttributeValue::Expression(_) => None,
        }
    })
}

/// Whether `text` names a known binding: prop, state, derived, local, or a
/// dotted path rooted at one of those.
fn is_path_reference(text: &str, script: &ValidatedScript, locals: &[String]) -> bool {
    let root = text.split('.').next().unwrap_or_default();
    script.props.iter().any(|prop| prop.name == root)
        || script.state.iter().any(|slot| slot.name == root)
        || script.derived.iter().any(|slot| slot.name == root)
        || locals.iter().any(|local| local == root)
}

/// Read a (possibly dotted) binding path, selecting the variant from the
/// binding root: props, state, and derived slots keep their kind; anything
/// else is a local. Dotted paths flatten so member access never needs a
/// separate IR node.
fn read_binding(script: &ValidatedScript, path: &str) -> StudioExpression {
    // Callers guard with `is_path_reference`, so every root here is a prop,
    // state slot, derived slot, or an in-scope local; anything else is a
    // local-shaped path by construction.
    let root = path.split('.').next().unwrap_or_default();
    if script.props.iter().any(|prop| prop.name == root) {
        StudioExpression::ReadProp(path.to_owned())
    } else if script.state.iter().any(|slot| slot.name == root) {
        StudioExpression::ReadState(path.to_owned())
    } else if script.derived.iter().any(|slot| slot.name == root) {
        StudioExpression::ReadDerived(path.to_owned())
    } else {
        StudioExpression::ReadLocal(path.to_owned())
    }
}

/// A plain function reference for handler attributes: `addToOrder`, never a
/// call or an inline arrow.
fn function_reference(expression: &SourceExpression) -> Option<String> {
    if expression.json.get("type").and_then(|kind| kind.as_str()) == Some("Identifier") {
        return expression
            .json
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_owned);
    }
    None
}

/// Classify a JSON literal node into a Studio value.
fn literal_from_json(json: &serde_json::Value, text: &str) -> Option<StudioValue> {
    let value = json.get("value")?;
    if let Some(text) = value.as_str() {
        return Some(StudioValue::String(text.to_owned()));
    }
    if let Some(boolean) = value.as_bool() {
        return Some(StudioValue::Boolean(boolean));
    }
    if let Some(number) = value.as_f64() {
        return Some(StudioValue::Number(crate::types::NumberLiteral {
            text: json
                .get("raw")
                .and_then(|raw| raw.as_str())
                .unwrap_or(text)
                .to_owned(),
            value: number,
        }));
    }
    if value.is_null() {
        return None;
    }
    None
}

/// Byte offset from one JSON node position, if it fits the address space.
fn json_offset(value: Option<&serde_json::Value>) -> Option<usize> {
    usize::try_from(value?.as_u64()?).ok()
}

/// Source text of one JSON node by its start/end offsets.
fn node_text<'a>(source: &'a str, json: &serde_json::Value) -> Option<&'a str> {
    let start = json_offset(json.get("start"))?;
    let end = json_offset(json.get("end"))?;
    if start <= end
        && end <= source.len()
        && source.is_char_boundary(start)
        && source.is_char_boundary(end)
    {
        Some(&source[start..end])
    } else {
        None
    }
}

/// Parse a static member path (`item.name`, `rows[0].id`) from source text.
fn member_path_from_text(text: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut rest = text.trim();
    let end = rest
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        })
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let (head, tail) = rest.split_at(end);
    if !head.chars().next().is_some_and(|character| {
        character.is_ascii_alphabetic() || character == '_' || character == '$'
    }) {
        return None;
    }
    parts.push(head.to_owned());
    rest = tail.trim_start();
    while !rest.is_empty() {
        if let Some(dotted) = rest.strip_prefix('.') {
            let dotted = dotted.trim_start();
            let end = dotted
                .find(|character: char| {
                    !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
                })
                .unwrap_or(dotted.len());
            if end == 0 {
                return None;
            }
            let (name, tail) = dotted.split_at(end);
            parts.push(name.to_owned());
            rest = tail.trim_start();
        } else if let Some(bracketed) = rest.strip_prefix('[') {
            let bracketed = bracketed.trim_start();
            let close = bracketed.find(']')?;
            let index = bracketed[..close].trim();
            let normalized = if index.parse::<u64>().is_ok() {
                index.to_owned()
            } else if index.len() >= 2
                && ((index.starts_with('"') && index.ends_with('"'))
                    || (index.starts_with('\'') && index.ends_with('\'')))
            {
                index[1..index.len() - 1].to_owned()
            } else {
                return None;
            };
            parts.push(normalized);
            rest = bracketed[close + 1..].trim_start();
        } else {
            return None;
        }
    }
    Some(parts.join("."))
}

/// Entries of an object literal: `(key, value-json)` pairs.
fn object_entries(json: &serde_json::Value) -> Option<Vec<(String, serde_json::Value)>> {
    let properties = json.get("properties")?.as_array()?;
    let mut entries = Vec::new();
    for property in properties {
        if property.get("type").and_then(|kind| kind.as_str()) == Some("SpreadElement") {
            return None;
        }
        let key = property.get("key")?;
        let name = match key.get("type").and_then(|kind| kind.as_str()) {
            Some("Identifier") => key.get("name")?.as_str()?.to_owned(),
            Some("Literal") => key.get("value")?.as_str()?.to_owned(),
            _ => return None,
        };
        entries.push((name, property.get("value")?.clone()));
    }
    Some(entries)
}

/// Content prop folded from children, if the kind renders leaf content.
pub(crate) fn content_prop(kind: &str) -> Option<&'static str> {
    match kind {
        "Text" => Some("text"),
        "Button" => Some("label"),
        _ => None,
    }
}

/// Fold ordered text runs into one concatenation expression.
fn fold_runs(runs: Vec<StudioExpression>) -> Option<StudioExpression> {
    let mut runs = runs.into_iter();
    let first = runs.next()?;
    Some(
        runs.fold(first, |accumulated, run| StudioExpression::Binary {
            operator: "+".to_owned(),
            left: Box::new(accumulated),
            right: Box::new(run),
        }),
    )
}

/// A prop binding as a value expression.
fn prop_binding_expression(binding: &PropBinding) -> StudioExpression {
    match &binding.value {
        PropValue::Literal(literal) => StudioExpression::Literal(literal.clone()),
        PropValue::Expression(expression) => expression.clone(),
    }
}

impl Lowerer<'_> {
    /// Fold one component's children into a content binding.
    ///
    /// Returns whether the children are represented (so the caller skips
    /// recursing into runs that never mount).
    fn fold_component_content(
        &mut self,
        id: &str,
        kind: &str,
        props: &[PropBinding],
        children: &[TemplateNode],
        span: Span,
        content: &mut Vec<crate::ir::ContentBinding>,
    ) -> bool {
        let Some(prop) = content_prop(kind) else {
            return false;
        };
        let mut runs = Vec::new();
        for child in children {
            match child {
                TemplateNode::Text { value, .. } => {
                    if !value.trim().is_empty() {
                        runs.push(StudioExpression::Literal(StudioValue::String(
                            value.clone(),
                        )));
                    }
                }
                TemplateNode::Interpolation { expression, .. } => {
                    runs.push(expression.clone());
                }
                _ => {
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!(
                            "`{kind}` cannot contain nested content; move it out of node `{id}`"
                        ),
                        span,
                    );
                    return false;
                }
            }
        }
        let explicit = props.iter().find(|binding| binding.name == prop);
        let expression = fold_runs(runs);
        match (expression, explicit) {
            (None, None) => false,
            (None, Some(binding)) => {
                content.push(crate::ir::ContentBinding {
                    node_id: id.to_owned(),
                    prop: prop.to_owned(),
                    expression: prop_binding_expression(binding),
                });
                true
            }
            (Some(_), Some(_)) => {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("`{prop}` prop with children is ambiguous on node `{id}`"),
                    span,
                );
                false
            }
            (Some(expression), None) => {
                content.push(crate::ir::ContentBinding {
                    node_id: id.to_owned(),
                    prop: prop.to_owned(),
                    expression,
                });
                true
            }
        }
    }
}
