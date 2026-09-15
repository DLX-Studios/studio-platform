//! Portable-subset and catalog validation over adapter output.
//!
//! The validator is the final authority before either backend: it extracts
//! the script model (`$props`, `$state`, `$derived`, functions with `emit`
//! calls), enforces the closed component catalog and the portable expression
//! surface, and rejects everything else with stable `STUDIO3xx` diagnostics.
//! Text scanning strips string literals first so quoted code-like content
//! never triggers the rune or denylist rules.

use crate::rsvelte_adapter::{
    SourceAttribute, SourceAttributeValue, SourceExpression, SourceModule, SourceNode,
    SourceScriptKind, SourceSpan, span_for,
};
use crate::types::{StudioType, StudioValue};
use crate::{
    CODE_BINDING, CODE_NO_LOWERING_RULE, CODE_NON_SERIALIZABLE, CODE_SHAPE, CODE_STUDIO_UNKNOWN,
    CODE_SUBSET_BOUNDARY, CODE_UNINFERRED_TYPE, CODE_UNKNOWN_CALL, CODE_UNSUPPORTED_RUNE,
    Diagnostic, Severity, Span,
};

/// One validated `$props()` entry.
#[derive(Clone, Debug)]
pub struct ScriptProp {
    /// Prop name.
    pub name: String,
    /// Default value; `None` means the prop is required.
    pub default: Option<StudioValue>,
    /// Whether callers must supply the prop.
    pub required: bool,
    /// Source span of the entry.
    pub span: Span,
}

/// One validated `$state()` slot.
#[derive(Clone, Debug)]
pub struct ScriptState {
    /// Binding name.
    pub name: String,
    /// Initializer literal.
    pub value: StudioValue,
    /// Closed type inferred from the initializer.
    pub ty: StudioType,
    /// Source span of the declaration.
    pub span: Span,
}

/// One validated `$derived()` slot.
#[derive(Clone, Debug)]
pub struct ScriptDerived {
    /// Binding name.
    pub name: String,
    /// Derivation expression text (classified by the lowerer).
    pub expr_text: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// One validated function with its extracted emissions.
#[derive(Clone, Debug)]
pub struct ScriptFunction {
    /// Function name.
    pub name: String,
    /// `emit()` calls in source order.
    pub emissions: Vec<Emission>,
    /// State mutations in source order.
    pub mutations: Vec<Mutation>,
    /// Source span of the declaration site.
    pub span: Span,
}

/// One state-slot mutation statement inside a handler body.
#[derive(Clone, Debug)]
pub struct Mutation {
    /// Target `$state` slot name.
    pub slot: String,
    /// Mutation operation.
    pub op: MutationOp,
    /// Operand expression text (empty for increment/decrement).
    pub operand_text: String,
    /// Source span of the statement.
    pub span: Span,
}

/// Supported state-mutation operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationOp {
    /// `slot = expr`.
    Assign,
    /// `slot += expr`.
    Add,
    /// `slot -= expr`.
    Sub,
    /// `slot++`.
    Inc,
    /// `slot--`.
    Dec,
    /// `slot.push(expr)`.
    Push,
    /// `slot.pop()`.
    Pop,
    /// `slot.remove(expr)`.
    Remove,
    /// `slot.clear()`.
    Clear,
}

/// One `emit("event", payload)` call.
#[derive(Clone, Debug)]
pub struct Emission {
    /// Emitted event name.
    pub event: String,
    /// Payload expression text.
    pub payload_text: String,
    /// Source span of the call.
    pub span: Span,
}

/// The validated script model consumed by the lowerer.
#[derive(Clone, Debug, Default)]
pub struct ValidatedScript {
    /// Props in declaration order.
    pub props: Vec<ScriptProp>,
    /// State slots in declaration order.
    pub state: Vec<ScriptState>,
    /// Derived slots in declaration order.
    pub derived: Vec<ScriptDerived>,
    /// Functions in declaration order.
    pub functions: Vec<ScriptFunction>,
}

/// Forbidden constructs: browser, Node/Bun, dynamic, async, reflection.
const DENYLIST: &[&str] = &[
    "document.",
    "window.",
    "navigator.",
    "location.",
    "fetch(",
    "eval(",
    "Function(",
    "process.",
    "require(",
    "import(",
    "await ",
    "async ",
    "new Promise",
    "prototype",
    "__proto__",
    "new Date(",
    "new Map(",
    "new Set(",
    "new WeakMap(",
    "setTimeout",
    "setInterval",
    "localStorage",
    "sessionStorage",
    "XMLHttpRequest",
    "WebSocket",
    "alert(",
    "confirm(",
    "prompt(",
];

/// Runes admitted by the portable subset.
const ALLOWED_RUNES: &[&str] = &["props", "state", "derived"];

/// Validate one parsed module against the portable subset and the closed
/// catalog.
///
/// # Errors
///
/// Returns every diagnostic when the module is invalid; no partial model.
pub fn validate_module(
    source: &str,
    module: &SourceModule,
) -> Result<ValidatedScript, Vec<Diagnostic>> {
    let mut worker = Validator {
        source,
        diagnostics: Vec::new(),
        script: ValidatedScript::default(),
        seen_ids: std::collections::BTreeSet::new(),
    };
    worker.validate(module);
    if worker.diagnostics.is_empty() {
        Ok(worker.script)
    } else {
        Err(worker.diagnostics)
    }
}

struct Validator<'a> {
    source: &'a str,
    diagnostics: Vec<Diagnostic>,
    script: ValidatedScript,
    seen_ids: std::collections::BTreeSet<String>,
}

impl Validator<'_> {
    fn validate(&mut self, module: &SourceModule) {
        for script in &module.scripts {
            self.validate_script_text(&script.content, script.content_span.start);
        }
        self.extract_script_model(module);
        for node in &module.template {
            self.validate_node(node);
        }
        self.validate_list_shapes(module);
    }

    fn error(&mut self, code: &'static str, message: String, start: u32, end: u32) {
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            message,
            span: span_for(self.source, start, end),
        });
    }

    fn validate_script_text(&mut self, content: &str, base: u32) {
        let stripped = strip_strings(content);
        for forbidden in DENYLIST {
            let mut search = 0usize;
            while let Some(found) = stripped[search..].find(forbidden) {
                let absolute = offset(base as usize + search + found);
                self.error(
                    CODE_SUBSET_BOUNDARY,
                    format!(
                        "`{}` is outside the portable subset",
                        forbidden.trim_end_matches('(')
                    ),
                    absolute,
                    absolute + offset(forbidden.len()),
                );
                search += found + forbidden.len();
            }
        }
        let mut search = 0usize;
        while let Some(found) = stripped[search..].find('$') {
            let absolute = search + found;
            let rest = &stripped[absolute + 1..];
            let end = rest
                .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .unwrap_or(rest.len());
            let rune = &rest[..end];
            if !rune.is_empty() && !ALLOWED_RUNES.contains(&rune) {
                let start = offset(base as usize + absolute);
                self.error(
                    CODE_UNSUPPORTED_RUNE,
                    format!("rune `${rune}` is not part of the portable subset"),
                    start,
                    start + offset(rune.len()) + 1,
                );
            }
            search = absolute + 1 + end.max(1);
        }
        if (content.contains(": any") || content.contains(":any"))
            && let Some(found) = content.find(": any").or_else(|| content.find(":any"))
        {
            let start = offset(base as usize + found);
            self.error(
                CODE_NON_SERIALIZABLE,
                "explicit `any` is outside the portable subset".to_owned(),
                start,
                start + 5,
            );
        }
    }

    fn extract_script_model(&mut self, module: &SourceModule) {
        for script in &module.scripts {
            let base = script.content_span.start;
            self.extract_props(&script.content, base, script.kind);
            self.extract_state(&script.content, base);
            self.extract_derived(&script.content, base);
            self.extract_functions(&script.content, base);
        }
    }

    fn extract_props(&mut self, content: &str, base: u32, kind: SourceScriptKind) {
        let mut search = 0usize;
        while let Some(found) = content[search..].find("$props()") {
            let absolute = search + found;
            let Some(let_start) = content[..absolute].rfind("let") else {
                search = absolute + 1;
                continue;
            };
            let destructured = &content[let_start + 3..absolute];
            let Some(open) = destructured.find('{') else {
                search = absolute + 1;
                continue;
            };
            let Some(close) = destructured.rfind('}') else {
                search = absolute + 1;
                continue;
            };
            let list_start = base as usize + let_start + 3 + open + 1;
            for entry in split_top_level(&destructured[open + 1..close], ',') {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                let (name, default) = match entry.split_once('=') {
                    Some((name, default)) => (name.trim(), Some(default.trim())),
                    None => (entry, None),
                };
                let Some(name) = valid_identifier(name) else {
                    self.error(
                        CODE_UNINFERRED_TYPE,
                        format!("invalid prop entry `{entry}`"),
                        offset(list_start),
                        offset(list_start) + offset(entry.len()),
                    );
                    continue;
                };
                let entry_offset = base as usize
                    + content[let_start..]
                        .find(name)
                        .map_or(let_start, |found| let_start + found);
                let span_start = offset(entry_offset);
                let value = match default {
                    None => None,
                    Some(text) => {
                        if let Some(value) = parse_literal(text) {
                            Some(value)
                        } else {
                            self.error(
                                CODE_UNINFERRED_TYPE,
                                format!("prop `{name}` default is not a portable literal"),
                                span_start,
                                span_start + offset(text.len()),
                            );
                            continue;
                        }
                    }
                };
                if kind == SourceScriptKind::Module {
                    self.error(
                        CODE_UNSUPPORTED_RUNE,
                        "`$props()` is instance-only and cannot appear in a module script"
                            .to_owned(),
                        span_start,
                        span_start + offset(name.len()),
                    );
                    continue;
                }
                self.script.props.push(ScriptProp {
                    name: name.to_owned(),
                    required: value.is_none(),
                    default: value,
                    span: span_for(self.source, span_start, span_start + offset(name.len())),
                });
            }
            search = absolute + 1;
        }
    }

    fn extract_state(&mut self, content: &str, base: u32) {
        for (name, init, start, end) in rune_calls(content, "$state") {
            match parse_literal(&init) {
                Some(value) => {
                    let ty = value.ty();
                    self.script.state.push(ScriptState {
                        name,
                        value,
                        ty,
                        span: span_for(self.source, base + start, base + end),
                    });
                }
                None => {
                    self.error(
                        CODE_UNINFERRED_TYPE,
                        "state initializers must be portable literals".to_owned(),
                        base + start,
                        base + end,
                    );
                }
            }
        }
    }

    fn extract_derived(&mut self, content: &str, base: u32) {
        for (name, expr, start, end) in rune_calls(content, "$derived") {
            if expr.trim().is_empty() {
                self.error(
                    CODE_UNINFERRED_TYPE,
                    format!("derived slot `{name}` needs an expression"),
                    base + start,
                    base + end,
                );
                continue;
            }
            self.script.derived.push(ScriptDerived {
                name,
                expr_text: expr,
                span: span_for(self.source, base + start, base + end),
            });
        }
    }

    fn extract_functions(&mut self, content: &str, base: u32) {
        let mut search = 0usize;
        while let Some(found) = content[search..].find("function ") {
            let absolute = search + found;
            let rest = &content[absolute + 9..];
            let end = rest
                .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .unwrap_or(rest.len());
            let name = rest[..end].to_owned();
            if name.is_empty() {
                search = absolute + 1;
                continue;
            }
            let body_start = absolute + 9 + end;
            let next_function = content[body_start..]
                .find("function ")
                .map_or(content.len(), |found| body_start + found);
            let mut emissions = Vec::new();
            // Handler bodies run brace-delimited: the scan window starts at
            // the function name, so strip the signature and outer braces to
            // keep statement splitting from seeing parameter lists.
            let window = &content[body_start..next_function];
            let body = window.find('{').map_or(window, |open| {
                let inner = &window[open + 1..];
                inner.rfind('}').map_or(inner, |close| &inner[..close])
            });
            let body_base = base
                + u32::try_from(body_start).unwrap_or(u32::MAX)
                + window.find('{').map_or(0, |open| {
                    u32::try_from(open).unwrap_or(u32::MAX).saturating_add(1)
                });
            let mutations = self.extract_mutations(body, body_base);
            let mut emit_search = body_start;
            while let Some(found) = content[emit_search..next_function].find("emit(") {
                let call = emit_search + found;
                let call_text = content[call..]
                    .split_at(
                        content[call..]
                            .len()
                            .min(next_function.saturating_sub(call)),
                    )
                    .0;
                let _ = next_function;
                let Some((event, payload)) = parse_emit_arguments(call_text) else {
                    self.error(
                        CODE_UNKNOWN_CALL,
                        "emit() needs a string event name".to_owned(),
                        offset(base as usize + call),
                        offset(base as usize + call + 5),
                    );
                    break;
                };
                emissions.push(Emission {
                    event,
                    payload_text: payload,
                    span: span_for(
                        self.source,
                        offset(base as usize + call),
                        offset(base as usize + call + 5),
                    ),
                });
                emit_search = call + 5;
            }
            self.script.functions.push(ScriptFunction {
                name,
                emissions,
                mutations,
                span: span_for(
                    self.source,
                    offset(base as usize + absolute),
                    offset(base as usize + absolute + 9 + end),
                ),
            });
            search = next_function.max(absolute + 1);
        }
    }

    /// Extract `$state` mutation statements from one handler body.
    ///
    /// Supported statements: `slot = expr`, `slot += expr`, `slot -= expr`,
    /// `slot++`, `slot--`. Anything else that is not an `emit()` call or a
    /// bare brace is ignored here (other validators own those rules).
    fn extract_mutations(&mut self, body: &str, base: u32) -> Vec<Mutation> {
        let mut mutations = Vec::new();
        let mut cursor = 0usize;
        for statement in body.split(';') {
            let statement_offset = cursor;
            cursor += statement.len() + 1;
            let trimmed = statement.trim();
            if trimmed.is_empty()
                || trimmed == "{"
                || trimmed == "}"
                || trimmed.starts_with("emit(")
                || trimmed.starts_with("function ")
            {
                continue;
            }
            let start = offset(base as usize + statement_offset);
            let end = offset(base as usize + statement_offset + statement.len());
            if let Some((slot, method, args)) = parse_list_mutation(trimmed) {
                self.extract_list_mutation(&slot, &method, &args, start, end, &mut mutations);
                continue;
            }
            let Some((slot, op, operand)) = parse_mutation_statement(trimmed) else {
                continue;
            };
            if self
                .script
                .derived
                .iter()
                .any(|derived| derived.name == slot)
            {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("cannot mutate derived slot `{slot}`"),
                    start,
                    end,
                );
                continue;
            }
            if self.script.props.iter().any(|prop| prop.name == slot) {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("cannot mutate prop `{slot}`"),
                    start,
                    end,
                );
                continue;
            }
            if !self.script.state.iter().any(|state| state.name == slot) {
                self.error(
                    CODE_BINDING,
                    format!("`{slot}` is not a `$state` slot"),
                    start,
                    end,
                );
                continue;
            }
            if matches!(op, MutationOp::Assign | MutationOp::Add | MutationOp::Sub)
                && operand.trim().is_empty()
            {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("mutation of `{slot}` needs an operand"),
                    start,
                    end,
                );
                continue;
            }
            mutations.push(Mutation {
                slot,
                op,
                operand_text: operand.clone(),
                span: span_for(self.source, start, end),
            });
        }
        mutations
    }

    /// Extract one list mutation after the slot is known to be array state.
    fn extract_list_mutation(
        &mut self,
        slot: &str,
        method: &str,
        args: &str,
        start: u32,
        end: u32,
        mutations: &mut Vec<Mutation>,
    ) {
        let slot_is_state = self.script.state.iter().any(|state| state.name == slot);
        if !slot_is_state {
            self.error(
                CODE_BINDING,
                format!("`{slot}` is not a `$state` slot"),
                start,
                end,
            );
            return;
        }
        let is_array = self
            .script
            .state
            .iter()
            .find(|state| state.name == slot)
            .is_some_and(|state| matches!(state.value, StudioValue::Array(_)));
        if !is_array {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("`{method}` needs an array `$state` slot, `{slot}` is not one"),
                start,
                end,
            );
            return;
        }
        let (op, operand_text) = match method {
            "push" | "remove" if args.is_empty() => {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("`{method}` needs an argument"),
                    start,
                    end,
                );
                return;
            }
            "pop" | "clear" if !args.is_empty() => {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    format!("`{method}` takes no arguments"),
                    start,
                    end,
                );
                return;
            }
            "push" => (MutationOp::Push, args.to_owned()),
            "remove" => (MutationOp::Remove, args.to_owned()),
            "pop" => (MutationOp::Pop, String::new()),
            "clear" => (MutationOp::Clear, String::new()),
            _ => return,
        };
        mutations.push(Mutation {
            slot: slot.to_owned(),
            op,
            operand_text,
            span: span_for(self.source, start, end),
        });
    }

    /// Validate list shapes across handlers and iteration blocks.
    ///
    /// Record shapes must agree between literal initializers and push
    /// operands (scalar fields only); dynamic blocks need bare state
    /// collections, static row structure, and covered item reads.
    #[allow(clippy::too_many_lines)]
    fn validate_list_shapes(&mut self, module: &SourceModule) {
        enum PushEvidence {
            Literal {
                slot: String,
                keys: Vec<String>,
                start: u32,
                end: u32,
            },
            NotLiteral {
                slot: String,
                start: u32,
                end: u32,
            },
        }
        let mutated = self.mutated_slots();
        // Snapshot array shapes first: reporting borrows the validator
        // mutably while the script model is borrowed immutably.
        let mut shapes: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        let mut initial_shapes: Vec<(String, Vec<String>, bool, u32, u32)> = Vec::new();
        for slot in &self.script.state {
            let StudioValue::Array(elements) = &slot.value else {
                continue;
            };
            let mut agreed: Option<Vec<String>> = None;
            let mut mixed = false;
            for element in elements {
                let StudioValue::Record(fields) = element else {
                    continue;
                };
                let mut names: Vec<String> = fields.keys().cloned().collect();
                names.sort();
                let scalar = names.iter().all(|name| {
                    matches!(
                        fields.get(name),
                        Some(
                            StudioValue::String(_)
                                | StudioValue::Number(_)
                                | StudioValue::Boolean(_)
                        )
                    )
                });
                if !scalar {
                    continue;
                }
                match &agreed {
                    Some(known) if *known != names => {
                        mixed = true;
                    }
                    Some(_) => {}
                    None => {
                        agreed = Some(names);
                    }
                }
            }
            let start = u32::try_from(slot.span.start.offset).unwrap_or(u32::MAX);
            let end = u32::try_from(slot.span.end.offset).unwrap_or(u32::MAX);
            initial_shapes.push((
                slot.name.clone(),
                agreed.unwrap_or_default(),
                mixed,
                start,
                end,
            ));
        }
        for (name, names, mixed, start, end) in initial_shapes {
            if mixed {
                self.error(
                    CODE_SHAPE,
                    format!("array `{name}` mixes record shapes; keep one field set"),
                    start,
                    end,
                );
                continue;
            }
            if names.is_empty() {
                continue;
            }
            shapes.insert(name, names);
        }
        // Push operands join the shape evidence; all must agree.
        // Snapshot first: reporting borrows mutably mid-iteration.
        let mut pushes = Vec::new();
        for function in &self.script.functions {
            for mutation in &function.mutations {
                if mutation.op != MutationOp::Push {
                    continue;
                }
                let start = u32::try_from(mutation.span.start.offset).unwrap_or(u32::MAX);
                let end = u32::try_from(mutation.span.end.offset).unwrap_or(u32::MAX);
                match record_keys(&mutation.operand_text) {
                    Some(keys) => pushes.push(PushEvidence::Literal {
                        slot: mutation.slot.clone(),
                        keys,
                        start,
                        end,
                    }),
                    None => pushes.push(PushEvidence::NotLiteral {
                        slot: mutation.slot.clone(),
                        start,
                        end,
                    }),
                }
            }
        }
        for push in pushes {
            match push {
                PushEvidence::NotLiteral { slot, start, end } => {
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!("push into `{slot}` needs a record literal"),
                        start,
                        end,
                    );
                }
                PushEvidence::Literal {
                    slot,
                    keys,
                    start,
                    end,
                } => match shapes.get(&slot) {
                    Some(known) if *known != keys => {
                        self.error(
                            CODE_SHAPE,
                            format!("push into `{slot}` changes the record shape"),
                            start,
                            end,
                        );
                    }
                    Some(_) => {}
                    None => {
                        shapes.insert(slot, keys);
                    }
                },
            }
        }
        self.validate_each_blocks(module, &mutated, &shapes);
    }

    /// Slots with any mutation in any handler body.
    fn mutated_slots(&self) -> Vec<String> {
        let mut slots = Vec::new();
        for function in &self.script.functions {
            for mutation in &function.mutations {
                if !slots.contains(&mutation.slot) {
                    slots.push(mutation.slot.clone());
                }
            }
        }
        slots
    }

    /// Check every iteration block against its collection shape.
    fn validate_each_blocks(
        &mut self,
        module: &SourceModule,
        mutated: &[String],
        shapes: &std::collections::BTreeMap<String, Vec<String>>,
    ) {
        // Ancestry context: patched rows cannot live in conditional
        // branches or nest, so the walk tracks both.
        let mut stack: Vec<(&SourceNode, bool, bool)> = module
            .template
            .iter()
            .map(|node| (node, false, false))
            .collect();
        while let Some((node, in_branch, outer_dynamic)) = stack.pop() {
            match node {
                SourceNode::Each {
                    collection,
                    item,
                    index,
                    key,
                    body,
                    fallback,
                    span,
                } => {
                    let dynamic = block_is_dynamic(&collection.text, mutated);
                    if dynamic && in_branch {
                        self.error(
                            CODE_NO_LOWERING_RULE,
                            "patched lists cannot live in conditional branches".to_owned(),
                            span.start,
                            span.end,
                        );
                    }
                    if dynamic && outer_dynamic {
                        self.error(
                            CODE_NO_LOWERING_RULE,
                            "patched lists cannot nest".to_owned(),
                            span.start,
                            span.end,
                        );
                    } else if outer_dynamic {
                        self.error(
                            CODE_NO_LOWERING_RULE,
                            "lists cannot nest inside patched rows".to_owned(),
                            span.start,
                            span.end,
                        );
                    }
                    let dynamic = block_is_dynamic(&collection.text, mutated);
                    self.validate_each_block(
                        collection,
                        item,
                        index.as_ref(),
                        key.as_ref(),
                        body,
                        fallback,
                        *span,
                        dynamic,
                        shapes,
                    );
                    let nested = outer_dynamic || dynamic;
                    stack.extend(
                        body.iter()
                            .chain(fallback.iter())
                            .map(|child| (child, in_branch, nested)),
                    );
                }
                SourceNode::Component { children, .. } => {
                    stack.extend(
                        children
                            .iter()
                            .map(|child| (child, in_branch, outer_dynamic)),
                    );
                }
                SourceNode::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    stack.extend(
                        consequent
                            .iter()
                            .chain(alternate.iter())
                            .map(|child| (child, true, outer_dynamic)),
                    );
                }
                SourceNode::Text { .. }
                | SourceNode::Interpolation(_)
                | SourceNode::Element { .. }
                | SourceNode::Unsupported { .. } => {}
            }
        }
    }

    /// Check one iteration block: collection form, key coverage, row
    /// structure, fallback purity, and read coverage.
    #[allow(clippy::too_many_arguments)]
    fn validate_each_block(
        &mut self,
        collection: &SourceExpression,
        item: &str,
        index: Option<&String>,
        key: Option<&SourceExpression>,
        body: &[SourceNode],
        fallback: &[SourceNode],
        span: SourceSpan,
        dynamic: bool,
        shapes: &std::collections::BTreeMap<String, Vec<String>>,
    ) {
        let collection_text = collection.text.trim().to_owned();
        // Duplicate static keys fail fast when the key is a simple field.
        // Snapshot first: reporting borrows mutably mid-iteration.
        let mut duplicates = Vec::new();
        if let Some(key) = key
            && let Some(field) = simple_key_field(&key.text, item)
            && let Some(slot) = self
                .script
                .state
                .iter()
                .find(|slot| slot.name == collection_text)
            && let StudioValue::Array(elements) = &slot.value
        {
            let mut seen = std::collections::BTreeSet::new();
            for element in elements {
                if let StudioValue::Record(fields) = element
                    && let Some(value) = fields.get(&field)
                {
                    let key_text = match value {
                        StudioValue::String(text) => text.clone(),
                        StudioValue::Number(literal) => literal.text.clone(),
                        StudioValue::Boolean(value) => value.to_string(),
                        StudioValue::Array(_) | StudioValue::Record(_) => continue,
                    };
                    if !seen.insert(key_text.clone()) {
                        duplicates.push((slot.name.clone(), key_text));
                    }
                }
            }
        }
        for (slot, key_text) in duplicates {
            self.error(
                CODE_SHAPE,
                format!("duplicate key `{key_text}` in `{slot}`"),
                span.start,
                span.end,
            );
        }
        if !dynamic {
            return;
        }
        // Dynamic collections iterate bare state slots only.
        if self
            .script
            .state
            .iter()
            .all(|slot| slot.name != collection_text)
        {
            self.error(
                CODE_NO_LOWERING_RULE,
                format!("dynamic collection `{collection_text}` needs a `$state` slot"),
                span.start,
                span.end,
            );
            return;
        }
        // Dynamic rows are static structure: no nested blocks.
        if has_nested_blocks(body) {
            self.error(
                CODE_NO_LOWERING_RULE,
                "patched rows cannot nest `{#if}` or `{#each}` blocks".to_owned(),
                span.start,
                span.end,
            );
        }
        // Fallbacks never see item or index locals.
        let index_name = index.map_or("index", |name| name.as_str());
        if reads_any(fallback, item, index_name) {
            self.error(
                CODE_NO_LOWERING_RULE,
                "fallbacks cannot read item or index locals".to_owned(),
                span.start,
                span.end,
            );
        }
        // Item reads need shape evidence covering every field.
        let reads = item_reads(body, item);
        if !reads.is_empty() {
            match shapes.get(&collection_text) {
                Some(fields) => {
                    let mut missing: Vec<&String> = reads
                        .iter()
                        .filter(|field| !fields.contains(field))
                        .collect();
                    missing.sort();
                    if let Some(field) = missing.first() {
                        self.error(
                            CODE_SHAPE,
                            format!("item `{field}` is not in the shape of `{collection_text}`"),
                            span.start,
                            span.end,
                        );
                    }
                }
                None => {
                    self.error(
                        CODE_SHAPE,
                        format!("cannot fix the item shape for `{collection_text}`"),
                        span.start,
                        span.end,
                    );
                }
            }
        }
    }

    fn validate_node(&mut self, node: &SourceNode) {
        match node {
            SourceNode::Component {
                name,
                attributes,
                children,
                span,
            } => {
                if is_catalog_kind(name) {
                    self.validate_shape(name, attributes, children, *span);
                } else {
                    self.error(
                        CODE_STUDIO_UNKNOWN,
                        format!("component `{name}` is not in the Studio catalog"),
                        span.start,
                        span.end,
                    );
                }
                self.validate_attributes(attributes, name);
                for child in children {
                    self.validate_node(child);
                }
            }
            SourceNode::Element { name, span, .. } => {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!("HTML element `<{name}>` is outside the Studio catalog"),
                    span.start,
                    span.end,
                );
            }
            SourceNode::Text { .. } | SourceNode::Interpolation(_) => {}
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                for child in consequent.iter().chain(alternate.iter()) {
                    self.validate_node(child);
                }
            }
            SourceNode::Each { body, fallback, .. } => {
                for child in body.iter().chain(fallback.iter()) {
                    self.validate_node(child);
                }
            }
            SourceNode::Unsupported { span, kind } => {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!("`{kind}` is not supported in Studio Script"),
                    span.start,
                    span.end,
                );
            }
        }
        // Keyed iteration requires a key expression; checked structurally here.
        if let SourceNode::Each {
            key: None, span, ..
        } = node
        {
            self.error(
                CODE_STUDIO_UNKNOWN,
                "keyed iteration requires a key expression".to_owned(),
                span.start,
                span.end,
            );
        }
    }

    /// Check one catalog component's mount shape: known props, leaf
    /// content rules, and container cardinality. Dynamic (`If`/`Each`)
    /// children make counts unknowable, so count checks skip those nodes;
    /// value checking stays a mount-time concern.
    fn validate_shape(
        &mut self,
        name: &str,
        attributes: &[SourceAttribute],
        children: &[SourceNode],
        span: crate::rsvelte_adapter::SourceSpan,
    ) {
        let converted = crate::lower::catalog_kind_name(name);
        let Ok(kind) =
            serde_json::from_str::<studio_protocol::NodeKind>(&format!("\"{converted}\""))
        else {
            return;
        };
        let limits = studio_protocol::ProtocolLimits::default();
        for attribute in attributes {
            let attribute_name = attribute.name.as_str();
            // Identity, events, and already-rejected directives never reach
            // the mount as props; each has its own rule elsewhere.
            if attribute_name == "id"
                || attribute_name == "key"
                || attribute_name == "{...spread}"
                || attribute_name.starts_with("on:")
                || attribute_name.starts_with("bind:")
                || is_event_attribute(attribute_name)
            {
                continue;
            }
            if !studio_protocol::is_known_property(kind, attribute_name, limits) {
                self.error(
                    CODE_SHAPE,
                    format!("property `{attribute_name}` is not known for `{name}`"),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
        }
        self.validate_child_shape(name, kind, children, span);
        // A content prop beside children is ambiguous: the fold cannot tell
        // which one the author meant.
        if let Some(prop) = crate::lower_studio::content_prop(name) {
            let explicit = attributes.iter().any(|attribute| attribute.name == prop);
            let has_runs = children.iter().any(|child| match child {
                SourceNode::Text { value, .. } => !value.trim().is_empty(),
                SourceNode::Interpolation(_) => true,
                _ => false,
            });
            if explicit && has_runs {
                self.error(
                    CODE_SHAPE,
                    format!("`{prop}` prop with children is ambiguous on `{name}`"),
                    span.start,
                    span.end,
                );
            }
        }
    }

    /// Check one component's children against its cardinality contract.
    fn validate_child_shape(
        &mut self,
        name: &str,
        kind: studio_protocol::NodeKind,
        children: &[SourceNode],
        span: crate::rsvelte_adapter::SourceSpan,
    ) {
        let mut elements = 0usize;
        let mut dynamic = false;
        let mut first_nested: Option<crate::rsvelte_adapter::SourceSpan> = None;
        for child in children {
            let child_span = match child {
                SourceNode::Component { span, .. }
                | SourceNode::Element { span, .. }
                | SourceNode::If { span, .. }
                | SourceNode::Each { span, .. } => Some(*span),
                SourceNode::Text { .. }
                | SourceNode::Interpolation(_)
                | SourceNode::Unsupported { .. } => None,
            };
            match child {
                SourceNode::Component { .. } | SourceNode::Element { .. } => {
                    elements += 1;
                }
                SourceNode::If { .. } | SourceNode::Each { .. } => {
                    dynamic = true;
                }
                SourceNode::Text { .. }
                | SourceNode::Interpolation(_)
                | SourceNode::Unsupported { .. } => {}
            }
            if first_nested.is_none() {
                first_nested = child_span;
            }
        }
        match studio_protocol::child_cardinality(kind) {
            studio_protocol::ChildCardinality::Any => {}
            studio_protocol::ChildCardinality::None => {
                if let Some(nested) = first_nested {
                    self.error(
                        CODE_SHAPE,
                        format!("`{name}` cannot contain nested content"),
                        nested.start,
                        nested.end,
                    );
                }
            }
            studio_protocol::ChildCardinality::AtMostOne => {
                if !dynamic && elements > 1 {
                    self.error(
                        CODE_SHAPE,
                        format!("`{name}` takes at most one child; wrap them in a `Column`"),
                        span.start,
                        span.end,
                    );
                }
            }
            studio_protocol::ChildCardinality::ExactlyOne => {
                if !dynamic && elements != 1 {
                    self.error(
                        CODE_SHAPE,
                        format!("`{name}` takes exactly one child"),
                        span.start,
                        span.end,
                    );
                }
            }
        }
    }

    fn validate_attributes(&mut self, attributes: &[SourceAttribute], component: &str) {
        let _ = component;
        for attribute in attributes {
            match &attribute.value {
                SourceAttributeValue::Expression(expression) if expression.text.is_empty() => {
                    self.error(
                        CODE_NO_LOWERING_RULE,
                        format!("empty expression for `{}`", attribute.name),
                        attribute.span.start,
                        attribute.span.end,
                    );
                }
                _ => {}
            }
            if attribute.name.starts_with("on:") || attribute.name.starts_with("bind:") {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!(
                        "directive `{}` is not supported in Studio Script",
                        attribute.name
                    ),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
            if attribute.name == "{...spread}" {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    "spread attributes are not supported in Studio Script".to_owned(),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
            if is_event_attribute(&attribute.name) && !is_supported_event(&attribute.name) {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!("event `{}` is not a Studio event", attribute.name),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
            if attribute.name == "key" {
                self.error(
                    CODE_NO_LOWERING_RULE,
                    "`key` attributes are not supported on components; key the enclosing each block".to_owned(),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
            if attribute.name == "id"
                && let Some(id) = id_string(&attribute.value)
                && !self.seen_ids.insert(id.clone())
            {
                self.error(
                    CODE_STUDIO_UNKNOWN,
                    format!("duplicate explicit id `{id}`"),
                    attribute.span.start,
                    attribute.span.end,
                );
            }
        }
    }
}

/// Read a static string id from an `id` attribute value, if it is one.
fn id_string(value: &SourceAttributeValue) -> Option<String> {
    match value {
        SourceAttributeValue::Sequence(parts) if parts.len() == 1 => match &parts[0] {
            crate::rsvelte_adapter::SourceAttributePart::Text(text) => Some(text.clone()),
            crate::rsvelte_adapter::SourceAttributePart::Interpolation(_) => None,
        },
        SourceAttributeValue::Sequence(_)
        | SourceAttributeValue::True
        | SourceAttributeValue::Expression(_) => None,
    }
}

/// Event attributes map to the three trigger families; anything else is out.
fn is_event_attribute(name: &str) -> bool {
    name.starts_with("on") && name.len() > 2 && !name.contains(':')
}

/// Supported event attributes and their trigger families.
fn is_supported_event(name: &str) -> bool {
    matches!(name, "onclick" | "onchange" | "onsubmit")
}

/// Map an event attribute to its trigger family.
#[must_use]
pub fn trigger_for_event(name: &str) -> Option<&'static str> {
    match name {
        "onclick" => Some("pressed"),
        "onchange" => Some("changed"),
        "onsubmit" => Some("submitted"),
        _ => None,
    }
}

/// Whether a tag names a closed catalog kind (`PascalCase` accepted).
fn is_catalog_kind(name: &str) -> bool {
    let converted = crate::lower::catalog_kind_name(name);
    let quoted = format!("\"{converted}\"");
    serde_json::from_str::<studio_protocol::NodeKind>(&quoted).is_ok()
}

/// Saturating byte-offset conversion for diagnostics (sources fit in memory).
fn offset(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Strip single, double, and backtick string literals for safe scanning.
/// Documented limitation: escaped quotes inside strings end the stripped
/// region early, which can only produce extra diagnostics, never misses on
/// real code outside strings.
fn strip_strings(content: &str) -> String {
    let mut stripped = String::with_capacity(content.len());
    let mut characters = content.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(character, '"' | '\'' | '`') {
            stripped.push(' ');
            while let Some(next) = characters.next() {
                if next == '\\' {
                    stripped.push(' ');
                    if characters.next().is_some() {
                        stripped.push(' ');
                    }
                    continue;
                }
                if next == character {
                    break;
                }
                stripped.push(' ');
            }
            stripped.push(' ');
        } else {
            stripped.push(character);
        }
    }
    stripped
}

/// Split on a delimiter ignoring nesting, strings, and template literals.
pub(crate) fn split_top_level(text: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(character, '"' | '\'' | '`') {
            current.push(character);
            while let Some(next) = characters.next() {
                current.push(next);
                if next == '\\' {
                    if let Some(escaped) = characters.next() {
                        current.push(escaped);
                    }
                    continue;
                }
                if next == character {
                    break;
                }
            }
            continue;
        }
        match character {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(character);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            _ if character == delimiter && depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
}

fn valid_identifier(name: &str) -> Option<&str> {
    let mut characters = name.chars();
    let first = characters.next()?;
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return None;
    }
    if characters
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '$')
    {
        Some(name)
    } else {
        None
    }
}

/// Find `let <name> = $rune(...)` calls with balanced argument capture.
/// Returns (name, argument text, absolute start, absolute end) in content bytes.
fn rune_calls(content: &str, rune: &str) -> Vec<(String, String, u32, u32)> {
    let mut calls = Vec::new();
    let mut search = 0usize;
    let marker = format!("{rune}(");
    while let Some(found) = content[search..].find(marker.as_str()) {
        let absolute = search + found;
        let Some(let_start) = content[..absolute].rfind("let ") else {
            search = absolute + 1;
            continue;
        };
        let declaration = content[let_start + 4..absolute].trim();
        let Some(name) = declaration
            .split(['=', ':'])
            .next()
            .map(str::trim)
            .filter(|name| valid_identifier(name).is_some())
        else {
            search = absolute + 1;
            continue;
        };
        let arguments_start = absolute + marker.len();
        let mut depth = 1usize;
        let mut index = arguments_start;
        let bytes = content.as_bytes();
        let mut in_string: Option<u8> = None;
        while index < bytes.len() && depth > 0 {
            let byte = bytes[index];
            if let Some(quote) = in_string {
                if byte == b'\\' {
                    index += 1;
                } else if byte == quote {
                    in_string = None;
                }
            } else if matches!(byte, b'"' | b'\'' | b'`') {
                in_string = Some(byte);
            } else if byte == b'(' {
                depth += 1;
            } else if byte == b')' {
                depth -= 1;
            }
            index += 1;
        }
        if depth == 0 {
            calls.push((
                name.to_owned(),
                content[arguments_start..index - 1].to_owned(),
                offset(absolute),
                offset(index),
            ));
        }
        search = absolute + 1;
    }
    calls
}

/// Parse a portable literal: strings, booleans, numbers, arrays, and records
/// composed of literals.
pub(crate) fn parse_literal(text: &str) -> Option<StudioValue> {
    use crate::types::{NumberLiteral, StudioValue};
    let text = text.trim();
    if text.len() >= 2
        && ((text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\'')))
    {
        return Some(StudioValue::String(text[1..text.len() - 1].to_owned()));
    }
    match text {
        "true" => return Some(StudioValue::Boolean(true)),
        "false" => return Some(StudioValue::Boolean(false)),
        _ => {}
    }
    if let Ok(value) = text.parse::<f64>()
        && (value.is_finite() || text.parse::<f64>().is_ok_and(f64::is_finite))
    {
        return Some(StudioValue::Number(NumberLiteral {
            text: text.to_owned(),
            value,
        }));
    }
    if text.starts_with('[') && text.ends_with(']') {
        let inner = text[1..text.len() - 1].trim();
        if inner.is_empty() {
            return None;
        }
        let mut elements = Vec::new();
        for part in split_top_level(inner, ',') {
            elements.push(parse_literal(part.trim())?);
        }
        let first = elements.first()?.ty();
        if elements.iter().all(|element| element.ty() == first) {
            return Some(StudioValue::Array(elements));
        }
        return None;
    }
    if text.starts_with('{') && text.ends_with('}') {
        let inner = text[1..text.len() - 1].trim();
        if inner.is_empty() {
            return None;
        }
        let mut fields = std::collections::BTreeMap::new();
        for part in split_top_level(inner, ',') {
            let (key, value) = part.split_once(':')?;
            let key = key.trim().trim_matches(['"', '\'']);
            valid_identifier(key)?;
            fields.insert(key.to_owned(), parse_literal(value.trim())?);
        }
        return Some(StudioValue::Record(fields));
    }
    None
}

/// Split the balanced argument list of one `emit(` call into its argument
/// texts. Returns `None` when the parentheses never balance.
fn emit_arguments(call: &str) -> Option<Vec<String>> {
    let mut depth = 0usize;
    let mut index = 0usize;
    let mut open = 0usize;
    let bytes = call.as_bytes();
    let mut in_string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            if byte == b'\\' {
                index += 1;
            } else if byte == quote {
                in_string = None;
            }
        } else if matches!(byte, b'"' | b'\'' | b'`') {
            in_string = Some(byte);
        } else if byte == b'(' {
            if depth == 0 {
                open = index;
            }
            depth += 1;
        } else if byte == b')' {
            depth -= 1;
            if depth == 0 {
                let inner = call.get(open + 1..index)?;
                return Some(split_top_level(inner, ','));
            }
        }
        index += 1;
    }
    None
}

fn parse_emit_arguments(call: &str) -> Option<(String, String)> {
    let mut parts = emit_arguments(call)?;
    if parts.is_empty() {
        return None;
    }
    let event = parts.remove(0);
    let event = event.trim();
    if event.len() >= 2 && event.starts_with('"') && event.ends_with('"') {
        let payload = parts.join(",").trim().to_owned();
        return Some((event[1..event.len() - 1].to_owned(), payload));
    }
    None
}

/// Split one handler-body statement into a state mutation, if it is one.
///
/// Returns the slot name, operation, and operand text (empty for `++`/`--`).
fn parse_mutation_statement(statement: &str) -> Option<(String, MutationOp, String)> {
    let trimmed = statement.trim().trim_end_matches('}').trim();
    for suffix in ["++", "--"] {
        if let Some(slot) = trimmed.strip_suffix(suffix) {
            let slot = slot.trim();
            if is_slot_name(slot) {
                let op = if suffix == "++" {
                    MutationOp::Inc
                } else {
                    MutationOp::Dec
                };
                return Some((slot.to_owned(), op, String::new()));
            }
            return None;
        }
    }
    for (marker, op) in [
        ("+=", MutationOp::Add),
        ("-=", MutationOp::Sub),
        ("=", MutationOp::Assign),
    ] {
        if let Some(index) = bare_assignment(statement, marker) {
            let (slot, operand) = statement.split_at(index);
            let slot = slot.trim();
            let operand = operand[marker.len()..].trim();
            if is_slot_name(slot) {
                return Some((slot.to_owned(), op, operand.to_owned()));
            }
            return None;
        }
    }
    None
}

/// Whether `text` is a plausible slot identifier.
fn is_slot_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$')
}

/// Find `marker` outside comparisons: skips `==`, `!=`, `<=`, `>=` pairings.
fn bare_assignment(statement: &str, marker: &str) -> Option<usize> {
    let bytes = statement.as_bytes();
    let mut index = 0usize;
    while index + marker.len() <= bytes.len() {
        if &statement[index..index + marker.len()] == marker {
            let before = index.checked_sub(1).map(|at| bytes[at]);
            let after = bytes.get(index + marker.len()).copied();
            let paired = before.is_some_and(|byte| matches!(byte, b'=' | b'!' | b'<' | b'>'))
                || after.is_some_and(|byte| matches!(byte, b'='));
            if !paired {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

/// Split one handler-body statement into a list mutation, if it is one.
///
/// Returns the slot name, method, and raw argument text.
fn parse_list_mutation(statement: &str) -> Option<(String, String, String)> {
    let trimmed = statement.trim().trim_end_matches('}').trim();
    let (receiver, call) = trimmed.split_once('.')?;
    let (method, rest) = call.split_once('(')?;
    if !is_slot_name(receiver.trim()) || !rest.trim_end().ends_with(')') {
        return None;
    }
    let method = method.trim();
    if !matches!(method, "push" | "pop" | "remove" | "clear") {
        return None;
    }
    let args = rest.trim_end();
    let args = args[..args.len() - 1].trim().to_owned();
    Some((receiver.trim().to_owned(), method.to_owned(), args))
}

/// Whether a collection expression reads a mutated slot.
fn block_is_dynamic(collection_text: &str, mutated: &[String]) -> bool {
    mutated
        .iter()
        .any(|slot| crate::lower_studio::contains_word(collection_text, slot))
}

/// Parse `{ key: value, ... }` top-level keys of a record literal.
fn record_keys(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut in_string: Option<u8> = None;
    let mut current = String::new();
    for byte in inner.bytes() {
        if let Some(quote) = in_string {
            current.push(byte as char);
            if byte == quote {
                in_string = None;
            }
            continue;
        }
        match byte {
            b'"' | b'\'' => {
                in_string = Some(byte);
                current.push(byte as char);
            }
            b'{' | b'[' => {
                depth += 1;
                current.push(byte as char);
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                current.push(byte as char);
            }
            b',' if depth == 0 => {
                keys.push(entry_key(&current)?);
                current.clear();
            }
            _ => current.push(byte as char),
        }
    }
    if !current.trim().is_empty() {
        keys.push(entry_key(&current)?);
    }
    keys.sort();
    Some(keys)
}

/// Split one `key: value` entry into its key name.
fn entry_key(entry: &str) -> Option<String> {
    let (key, _) = entry.split_once(':')?;
    let key = key.trim().trim_matches(['"', '\'']);
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    Some(key.to_owned())
}

/// Parse a simple key expression into its item field, if it is one.
///
/// Accepts `item.<field>` with optional surrounding parentheses; anything
/// else (bare `index`, complex expressions) evaluates at runtime and skips
/// static checks by returning `None`.
fn simple_key_field(text: &str, item: &str) -> Option<String> {
    let mut trimmed = text.trim();
    while trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2 {
        trimmed = trimmed[1..trimmed.len() - 1].trim();
    }
    let field = trimmed.strip_prefix(item)?.strip_prefix('.')?;
    if field.is_empty()
        || !field
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    Some(field.to_owned())
}

/// Whether any subtree node is an `{#if}` or `{#each}` block.
fn has_nested_blocks(nodes: &[SourceNode]) -> bool {
    let mut stack: Vec<&SourceNode> = nodes.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            SourceNode::If { .. } | SourceNode::Each { .. } => return true,
            SourceNode::Component { children, .. } => stack.extend(children.iter()),
            SourceNode::Text { .. }
            | SourceNode::Interpolation(_)
            | SourceNode::Element { .. }
            | SourceNode::Unsupported { .. } => {}
        }
    }
    false
}

/// Collect `item.<field>` reads across interpolations and attributes.
fn item_reads(nodes: &[SourceNode], item: &str) -> std::collections::BTreeSet<String> {
    let mut reads = std::collections::BTreeSet::new();
    let mut stack: Vec<&SourceNode> = nodes.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            SourceNode::Component {
                attributes,
                children,
                ..
            } => {
                for attribute in attributes {
                    collect_item_reads_in_text(&attribute_text(attribute), item, &mut reads);
                }
                stack.extend(children.iter());
            }
            SourceNode::Interpolation(expression) => {
                collect_item_reads_in_text(&expression.text, item, &mut reads);
            }
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                stack.extend(consequent.iter().chain(alternate.iter()));
            }
            SourceNode::Each { body, fallback, .. } => {
                stack.extend(body.iter().chain(fallback.iter()));
            }
            SourceNode::Text { .. }
            | SourceNode::Element { .. }
            | SourceNode::Unsupported { .. } => {}
        }
    }
    reads
}

/// Whether any text under the nodes mentions the item or index locals.
fn reads_any(nodes: &[SourceNode], item: &str, index: &str) -> bool {
    let mut stack: Vec<&SourceNode> = nodes.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            SourceNode::Component {
                attributes,
                children,
                ..
            } => {
                for attribute in attributes {
                    let text = attribute_text(attribute);
                    if crate::lower_studio::contains_word(&text, item)
                        || crate::lower_studio::contains_word(&text, index)
                    {
                        return true;
                    }
                }
                stack.extend(children.iter());
            }
            SourceNode::Interpolation(expression) => {
                if crate::lower_studio::contains_word(&expression.text, item)
                    || crate::lower_studio::contains_word(&expression.text, index)
                {
                    return true;
                }
            }
            SourceNode::If {
                consequent,
                alternate,
                ..
            } => {
                stack.extend(consequent.iter().chain(alternate.iter()));
            }
            SourceNode::Each { body, fallback, .. } => {
                stack.extend(body.iter().chain(fallback.iter()));
            }
            SourceNode::Text { .. }
            | SourceNode::Element { .. }
            | SourceNode::Unsupported { .. } => {}
        }
    }
    false
}

/// Collect `item.<field>` field reads from one expression text.
fn collect_item_reads_in_text(
    text: &str,
    item: &str,
    reads: &mut std::collections::BTreeSet<String>,
) {
    let prefix = format!("{item}.");
    let mut search = 0usize;
    while let Some(found) = text[search..].find(&prefix) {
        let absolute = search + found;
        let before = text[..absolute].chars().next_back();
        let boundary = before.is_none_or(|character| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        });
        if boundary {
            let rest = &text[absolute + prefix.len()..];
            let end = rest
                .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .unwrap_or(rest.len());
            if end > 0 {
                reads.insert(rest[..end].to_owned());
            }
        }
        search = absolute + prefix.len();
    }
}

/// Best-effort expression text of one attribute for read scanning.
fn attribute_text(attribute: &SourceAttribute) -> String {
    match &attribute.value {
        SourceAttributeValue::Expression(expression) => expression.text.clone(),
        SourceAttributeValue::Sequence(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::rsvelte_adapter::SourceAttributePart::Interpolation(expression) => {
                    Some(expression.text.clone())
                }
                crate::rsvelte_adapter::SourceAttributePart::Text(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        SourceAttributeValue::True => String::new(),
    }
}
