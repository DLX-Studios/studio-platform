//! Rust expression evaluator and template projector for Studio IR.
//!
//! The evaluator interprets v2 modules directly: expressions evaluate against
//! props, state, derived slots, and iteration locals; templates project onto
//! retained [`UiNode`] trees; handler dispatches return typed emissions. It
//! shares the exact registry/dispatcher/event vocabulary as the production
//! path, which is what makes differential equivalence meaningful.

use std::collections::BTreeMap;

use studio_protocol::{GuestMessage, MountTree, NodeKind, PatchBatch, PatchOp, UiNode};

use crate::ir::{
    ComponentDefinition, EventHandler, PropValue, StudioExpression, StudioModule, TemplateNode,
};
use crate::lower::catalog_kind_name;
use crate::types::StudioValue;

/// Failure evaluating or projecting Studio IR.
#[derive(Clone, Debug, PartialEq)]
pub struct EvalError {
    /// Stable code (reuses the `STUDIO320` lowering family: the construct has
    /// no evaluation rule in this context).
    pub code: &'static str,
    /// Safe message.
    pub message: String,
}

impl EvalError {
    fn rule(message: impl Into<String>) -> Self {
        Self {
            code: crate::CODE_NO_LOWERING_RULE,
            message: message.into(),
        }
    }
}

/// Live evaluation scope: props, state, and iteration locals.
#[derive(Clone, Debug, Default)]
pub struct EvalScope {
    /// Component prop values.
    pub props: BTreeMap<String, StudioValue>,
    /// State slot values.
    pub state: BTreeMap<String, StudioValue>,
    /// Derived slot values (recomputed by the caller before projecting).
    pub derived: BTreeMap<String, StudioValue>,
    /// Iteration and handler locals with dotted paths.
    pub locals: BTreeMap<String, StudioValue>,
}

impl EvalScope {
    /// Read one binding path, dotted paths included.
    #[must_use]
    pub fn read(&self, path: &str) -> Option<StudioValue> {
        let root = path.split('.').next().unwrap_or_default();
        let base = self
            .locals
            .get(root)
            .or_else(|| self.state.get(root))
            .or_else(|| self.props.get(root))
            .or_else(|| self.derived.get(root))?;
        if root.len() == path.len() {
            return Some(base.clone());
        }
        let mut current = base.clone();
        for segment in path[root.len() + 1..].split('.') {
            current = match current {
                StudioValue::Record(fields) => fields.get(segment)?.clone(),
                StudioValue::Array(elements) => {
                    elements.get(segment.parse::<usize>().ok()?)?.clone()
                }
                _ => return None,
            };
        }
        Some(current)
    }
}

/// Evaluate one closed expression in scope.
///
/// # Errors
///
/// Returns [`EvalError`] when a binding is unbound or an operation has no rule.
pub fn evaluate(
    expression: &StudioExpression,
    scope: &EvalScope,
) -> Result<StudioValue, EvalError> {
    match expression {
        StudioExpression::Literal(value) => Ok(value.clone()),
        StudioExpression::ReadProp(path)
        | StudioExpression::ReadState(path)
        | StudioExpression::ReadDerived(path)
        | StudioExpression::ReadLocal(path) => scope.read(path).ok_or_else(|| {
            EvalError::rule(format!("unbound reference `{path}` during evaluation"))
        }),
        StudioExpression::Unary { operator, operand } => {
            let value = evaluate(operand, scope)?;
            match operator.as_str() {
                "!" => Ok(StudioValue::Boolean(!truthy(&value))),
                "-" => Ok(number(-as_number(&value)?)),
                "+" => Ok(number(as_number(&value)?)),
                _ => Err(EvalError::rule(format!(
                    "unary operator `{operator}` is not supported"
                ))),
            }
        }
        StudioExpression::Binary {
            operator,
            left,
            right,
        } => evaluate_binary(operator, evaluate(left, scope)?, evaluate(right, scope)?),
        StudioExpression::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            if truthy(&evaluate(condition, scope)?) {
                evaluate(consequent, scope)
            } else {
                evaluate(alternate, scope)
            }
        }
        StudioExpression::Array(elements) => {
            let mut values = Vec::with_capacity(elements.len());
            for element in elements {
                values.push(evaluate(element, scope)?);
            }
            Ok(StudioValue::Array(values))
        }
        StudioExpression::Record(entries) => {
            let mut fields = BTreeMap::new();
            for (key, value) in entries {
                fields.insert(key.clone(), evaluate(value, scope)?);
            }
            Ok(StudioValue::Record(fields))
        }
        StudioExpression::CallApprovedFunction {
            function,
            arguments,
        } => {
            let mut values = Vec::with_capacity(arguments.len());
            for argument in arguments {
                values.push(evaluate(argument, scope)?);
            }
            call_approved(function, &values)
        }
    }
}

fn number(value: f64) -> StudioValue {
    StudioValue::Number(crate::types::NumberLiteral {
        text: value.to_string(),
        value,
    })
}

fn as_number(value: &StudioValue) -> Result<f64, EvalError> {
    match value {
        StudioValue::Number(literal) => Ok(literal.value),
        StudioValue::Boolean(value) => Ok(f64::from(u8::from(*value))),
        StudioValue::String(text) => text
            .parse::<f64>()
            .map_err(|_| EvalError::rule(format!("`{text}` is not numeric"))),
        _ => Err(EvalError::rule("value is not numeric")),
    }
}

pub(crate) fn as_text(value: &StudioValue) -> String {
    match value {
        StudioValue::String(text) => text.clone(),
        StudioValue::Boolean(value) => value.to_string(),
        StudioValue::Number(literal) => {
            if literal.value.fract() == 0.0 && literal.value.is_finite() {
                format!("{:.0}", literal.value)
            } else {
                literal.value.to_string()
            }
        }
        StudioValue::Array(_) | StudioValue::Record(_) => String::new(),
    }
}

fn truthy(value: &StudioValue) -> bool {
    match value {
        StudioValue::Boolean(value) => *value,
        StudioValue::Number(literal) => literal.value != 0.0,
        StudioValue::String(text) => !text.is_empty(),
        StudioValue::Array(elements) => !elements.is_empty(),
        StudioValue::Record(fields) => !fields.is_empty(),
    }
}

fn evaluate_binary(
    operator: &str,
    left: StudioValue,
    right: StudioValue,
) -> Result<StudioValue, EvalError> {
    match operator {
        "+" => {
            if matches!(left, StudioValue::String(_)) || matches!(right, StudioValue::String(_)) {
                Ok(StudioValue::String(as_text(&left) + &as_text(&right)))
            } else {
                Ok(number(as_number(&left)? + as_number(&right)?))
            }
        }
        "-" => Ok(number(as_number(&left)? - as_number(&right)?)),
        "*" => Ok(number(as_number(&left)? * as_number(&right)?)),
        "/" => {
            let divisor = as_number(&right)?;
            if divisor == 0.0 {
                return Err(EvalError::rule("division by zero"));
            }
            Ok(number(as_number(&left)? / divisor))
        }
        "%" => {
            let divisor = as_number(&right)?;
            if divisor == 0.0 {
                return Err(EvalError::rule("division by zero"));
            }
            Ok(number(as_number(&left)? % divisor))
        }
        "==" | "===" => Ok(StudioValue::Boolean(left == right)),
        "!=" | "!==" => Ok(StudioValue::Boolean(left != right)),
        "<" => Ok(StudioValue::Boolean(as_number(&left)? < as_number(&right)?)),
        ">" => Ok(StudioValue::Boolean(as_number(&left)? > as_number(&right)?)),
        "<=" => Ok(StudioValue::Boolean(
            as_number(&left)? <= as_number(&right)?,
        )),
        ">=" => Ok(StudioValue::Boolean(
            as_number(&left)? >= as_number(&right)?,
        )),
        "&&" => Ok(if truthy(&left) { right } else { left }),
        "||" => Ok(if truthy(&left) { left } else { right }),
        _ => Err(EvalError::rule(format!(
            "binary operator `{operator}` is not supported"
        ))),
    }
}

/// Approved pure functions callable from template expressions. The set is
/// intentionally tiny and closed; additions are additive IR-compatible changes.
fn call_approved(function: &str, arguments: &[StudioValue]) -> Result<StudioValue, EvalError> {
    match function {
        "formatMoney" => {
            let [minor] = arguments else {
                return Err(EvalError::rule("formatMoney needs exactly one argument"));
            };
            Ok(StudioValue::String(format_money(as_number(minor)?)))
        }
        _ => Err(EvalError::rule(format!(
            "`{function}` is not an approved function"
        ))),
    }
}

/// Format integer minor units as `D.CC`, mirroring the starter example's
/// `formatMinor` convention. Fractional input truncates toward zero.
#[must_use]
pub fn format_money(minor: f64) -> String {
    // Truncation and digit splitting stay in float arithmetic: values beyond
    // the exactly-representable integer range are already Decimal-bound by the
    // host mapping, and `formatMoney` documents truncation toward zero.
    let truncated = minor.trunc();
    let sign = if truncated.is_sign_negative() {
        "-"
    } else {
        ""
    };
    let units = truncated.abs();
    let dollars = (units / 100.0).trunc();
    let cents = units % 100.0;
    format!("{sign}{dollars:.0}.{cents:02.0}")
}

/// One typed emission produced by dispatching a handler.
#[derive(Clone, Debug, PartialEq)]
pub struct EmittedEvent {
    /// Emitted event name.
    pub name: String,
    /// Evaluated payload fields.
    pub payload: BTreeMap<String, StudioValue>,
}

impl EmittedEvent {
    /// Canonical JSON for differential comparison across backends:
    /// `{"type":"event","name":…,"payload":{…}}` with sorted keys.
    #[must_use]
    pub fn to_json(&self) -> String {
        let payload: BTreeMap<&str, serde_json::Value> = self
            .payload
            .iter()
            .map(|(key, value)| (key.as_str(), studio_value_json(value)))
            .collect();
        serde_json::json!({
            "type": "event",
            "name": self.name,
            "payload": payload,
        })
        .to_string()
    }
}

/// Mount a component: project its template onto a retained [`UiNode`] tree.
///
/// Props fall back to declared defaults; state slots start at their
/// initializers; derived slots evaluate in declaration order.
///
/// # Errors
///
/// Returns [`EvalError`] when a required prop is missing or an expression
/// has no evaluation rule.
pub fn mount_tree(
    module: &StudioModule,
    component: &ComponentDefinition,
    props: &BTreeMap<String, StudioValue>,
) -> Result<(studio_protocol::MountTree, EvalScope), EvalError> {
    if !module
        .components
        .iter()
        .any(|known| known.id == component.id)
    {
        return Err(EvalError::rule("component does not belong to the module"));
    }
    let mut scope = EvalScope::default();
    for prop in &component.props {
        if let Some(value) = props.get(&prop.name) {
            scope.props.insert(prop.name.clone(), value.clone());
        } else if let Some(default) = &prop.default {
            scope.props.insert(prop.name.clone(), default.clone());
        } else {
            return Err(EvalError::rule(format!(
                "missing required prop `{}`",
                prop.name
            )));
        }
    }
    for slot in &component.state {
        scope
            .state
            .insert(slot.name.clone(), evaluate(&slot.initial, &scope)?);
    }
    for slot in &component.derived {
        let value = evaluate(&slot.expression, &scope)?;
        scope.derived.insert(slot.name.clone(), value);
    }
    let route = format!("/{}", component.name);
    let root = project_root(component, &scope)?;
    Ok((
        MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route,
            root,
        },
        scope,
    ))
}

/// Project one component template against a live scope.
///
/// Shared by initial mounts and post-event re-projections so both observe
/// identical trees for identical scope contents.
///
/// # Errors
///
/// Returns [`EvalError`] when an expression has no evaluation rule.
fn project_root(component: &ComponentDefinition, scope: &EvalScope) -> Result<UiNode, EvalError> {
    let mut children = Vec::new();
    for node in &component.template {
        if let Some(ui) = project_node(node, scope)? {
            children.push(ui);
        }
    }
    Ok(UiNode {
        id: format!("{}#root", component.id),
        kind: NodeKind::Box,
        props: BTreeMap::new(),
        children,
    })
}

/// A live interaction session: mutable scope plus the guest-owned patch
/// sequence (first patch is 1, strictly increasing per call).
#[derive(Clone, Debug)]
pub struct InteractionSession {
    scope: EvalScope,
    sequence: u64,
}

/// One guest-call outcome: a host-applied patch or a designer event.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionMessage {
    /// Patch envelope for the native host.
    Patch(PatchBatch),
    /// Designer-channel event emission.
    Event(EmittedEvent),
}

impl SessionMessage {
    /// Serialize to the wire JSON the guest would emit.
    #[must_use]
    pub fn to_json(&self) -> String {
        match self {
            Self::Patch(batch) => serde_json::to_string(&GuestMessage::Patch(batch.clone()))
                .unwrap_or_else(|_| "{\"type\":\"patch\"}".to_owned()),
            Self::Event(emission) => emission.to_json(),
        }
    }
}

impl InteractionSession {
    /// Mount a component, returning the initial tree and session.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] exactly when [`mount_tree`] would.
    pub fn mount(
        module: &StudioModule,
        component: &ComponentDefinition,
        props: &BTreeMap<String, StudioValue>,
    ) -> Result<(studio_protocol::MountTree, Self), EvalError> {
        let (tree, scope) = mount_tree(module, component, props)?;
        Ok((tree, Self { scope, sequence: 0 }))
    }

    /// Apply one node event: run handler mutations, recompute derived slots,
    /// then return the patch (pure state handler) or the designer emission.
    ///
    /// Content props (`text` on `Text`, `label` on `Button`) re-project
    /// against updated state; every such node present in the tree yields one
    /// `update_prop` op, so eval and compiled legs agree byte for byte.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] for unknown handlers, unbound references, or
    /// ill-typed mutations.
    pub fn apply(
        &mut self,
        component: &ComponentDefinition,
        node_id: &str,
        event: &str,
    ) -> Result<SessionMessage, EvalError> {
        let Some(handler) = component
            .handlers
            .iter()
            .find(|handler| handler.node.as_deref() == Some(node_id) && handler.event == event)
        else {
            return Err(EvalError::rule(format!(
                "no handler for `{event}` on node `{node_id}`"
            )));
        };
        for mutation in &handler.mutations {
            apply_mutation(&mut self.scope, mutation)?;
        }
        for slot in &component.derived {
            let value = evaluate(&slot.expression, &self.scope)?;
            self.scope.derived.insert(slot.name.clone(), value);
        }
        if handler.emit.is_empty() {
            let tree = project_root(component, &self.scope)?;
            let mut operations = Vec::new();
            collect_structural_ops(component, handler, &tree, &mut operations);
            collect_content_ops(&tree, &mut operations);
            self.sequence += 1;
            Ok(SessionMessage::Patch(PatchBatch {
                sequence: self.sequence,
                operations,
            }))
        } else {
            let emission =
                dispatch_event(component, &self.scope, node_id, event, &component.handlers)?
                    .ok_or_else(|| EvalError::rule("handler stopped matching after mutation"))?;
            Ok(SessionMessage::Event(emission))
        }
    }
}

/// Apply one validated mutation to live state.
fn apply_mutation(scope: &mut EvalScope, mutation: &crate::ir::Mutation) -> Result<(), EvalError> {
    let Some(current) = scope.state.get(&mutation.slot).cloned() else {
        return Err(EvalError::rule(format!(
            "mutation targets unknown state slot `{}`",
            mutation.slot
        )));
    };
    let operand = if matches!(
        mutation.op,
        crate::ir::MutationOp::Inc | crate::ir::MutationOp::Dec
    ) {
        StudioValue::Number(crate::types::NumberLiteral {
            text: "1".to_owned(),
            value: 1.0,
        })
    } else {
        evaluate(&mutation.operand, scope)?
    };
    let updated = match mutation.op {
        crate::ir::MutationOp::Assign => operand,
        crate::ir::MutationOp::Add | crate::ir::MutationOp::Inc => {
            evaluate_binary("+", current.clone(), operand)?
        }
        crate::ir::MutationOp::Sub | crate::ir::MutationOp::Dec => {
            evaluate_binary("-", current.clone(), operand)?
        }
        crate::ir::MutationOp::Push
        | crate::ir::MutationOp::Pop
        | crate::ir::MutationOp::Remove
        | crate::ir::MutationOp::Clear => {
            return apply_list_mutation(scope, mutation, current, operand);
        }
    };
    if !same_slot_type(&current, &updated) {
        return Err(EvalError::rule(format!(
            "mutation of `{}` must keep the slot type",
            mutation.slot
        )));
    }
    scope.state.insert(mutation.slot.clone(), updated);
    Ok(())
}

/// Apply one list mutation: array slots only, out-of-range positions
/// are silent no-ops so both legs agree without error plumbing.
fn apply_list_mutation(
    scope: &mut EvalScope,
    mutation: &crate::ir::Mutation,
    current: StudioValue,
    operand: StudioValue,
) -> Result<(), EvalError> {
    let StudioValue::Array(mut elements) = current else {
        return Err(EvalError::rule(format!(
            "mutation of `{}` needs an array slot",
            mutation.slot
        )));
    };
    match mutation.op {
        crate::ir::MutationOp::Push => {
            elements.push(operand);
        }
        crate::ir::MutationOp::Pop => {
            elements.pop();
        }
        crate::ir::MutationOp::Remove => {
            if let Ok(index) = as_index(&operand)
                && index < elements.len()
            {
                elements.remove(index);
            }
        }
        crate::ir::MutationOp::Clear => {
            elements.clear();
        }
        _ => {
            return Err(EvalError::rule("not a list mutation"));
        }
    }
    scope
        .state
        .insert(mutation.slot.clone(), StudioValue::Array(elements));
    Ok(())
}

/// A non-negative integer position for list removal.
fn as_index(value: &StudioValue) -> Result<usize, EvalError> {
    match value {
        StudioValue::Number(literal) if literal.value >= 0.0 && literal.value.fract() == 0.0 =>
        {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "non-negative integral inputs saturate into a guarded range check"
            )]
            usize::try_from(literal.value as u64)
                .map_err(|_| EvalError::rule("list index out of range"))
        }
        _ => Err(EvalError::rule("list removal needs a numeric index")),
    }
}

/// Whether a mutation result keeps the slot's value family.
fn same_slot_type(before: &StudioValue, after: &StudioValue) -> bool {
    matches!(
        (before, after),
        (StudioValue::Boolean(_), StudioValue::Boolean(_))
            | (StudioValue::Number(_), StudioValue::Number(_))
            | (StudioValue::String(_), StudioValue::String(_))
            | (StudioValue::Array(_), StudioValue::Array(_))
            | (StudioValue::Record(_), StudioValue::Record(_))
    )
}

/// Collect `update_prop` ops for every content node in document order:
/// `text` on `Text`, `label` on `Button`, whenever the prop is present.
///
/// Branch (`#branch`) and row-list (`#items`) subtrees are skipped: their
/// content rides in `ReplaceNode` envelopes (or never patches), which is
/// what keeps the evaluator and compiled legs byte-identical.
fn collect_content_ops(node: &UiNode, operations: &mut Vec<PatchOp>) {
    if node.id.ends_with("#branch") || node.id.ends_with("#items") {
        return;
    }
    let prop = match node.kind {
        NodeKind::Text => Some("text"),
        NodeKind::Button => Some("label"),
        _ => None,
    };
    if let Some(prop) = prop
        && let Some(value) = node.props.get(prop)
    {
        operations.push(PatchOp::UpdateProp {
            node_id: node.id.clone(),
            property: prop.to_owned(),
            value: value.clone(),
        });
    }
    for child in &node.children {
        collect_content_ops(child, operations);
    }
}

/// Collect one `ReplaceNode` per row list whose collection slot the
/// handler mutated, sorted by wrapper id.
///
/// Sorting (rather than template order) keeps the two codegen legs
/// trivially shared: ops are id-addressed, so order across independent
/// wrappers is irrelevant to the host. The rebuilt wrapper subtree
/// carries fresh row content, so no separate content ops are needed.
fn collect_structural_ops(
    component: &ComponentDefinition,
    handler: &crate::ir::EventHandler,
    tree: &UiNode,
    operations: &mut Vec<PatchOp>,
) {
    let mutated: Vec<&str> = handler
        .mutations
        .iter()
        .filter(|mutation| {
            matches!(
                mutation.op,
                crate::ir::MutationOp::Push
                    | crate::ir::MutationOp::Pop
                    | crate::ir::MutationOp::Remove
                    | crate::ir::MutationOp::Clear
            )
        })
        .map(|mutation| mutation.slot.as_str())
        .collect();
    if mutated.is_empty() {
        return;
    }
    let before = operations.len();
    let mut stack: Vec<&crate::ir::TemplateNode> = component.template.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            TemplateNode::Each { id, collection, .. } => {
                // Dynamic blocks iterate a bare mutated state slot; anything
                // else is static (the validator rejects dynamic complexity).
                // This narrow rule mirrors the compiled backend exactly.
                if let StudioExpression::ReadState(slot) = collection
                    && mutated.contains(&slot.as_str())
                {
                    let wrapper = format!("{id}#items");
                    if let Some(subtree) = find_node(tree, &wrapper) {
                        operations.push(PatchOp::ReplaceNode {
                            node_id: wrapper,
                            node: subtree.clone(),
                        });
                    }
                }
            }
            TemplateNode::Component { children, .. } => {
                stack.extend(children.iter());
            }
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                stack.extend(consequent.iter().chain(alternate.iter()));
            }
            TemplateNode::Text { .. } | TemplateNode::Interpolation { .. } => {}
        }
    }
    operations[before..].sort_by(|left, right| {
        fn key(operation: &PatchOp) -> &str {
            match operation {
                PatchOp::ReplaceNode { node_id, .. } => node_id,
                _ => "",
            }
        }
        key(left).cmp(key(right))
    });
}

/// Find a projected node by id, depth first.
fn find_node<'a>(tree: &'a UiNode, id: &str) -> Option<&'a UiNode> {
    if tree.id == id {
        return Some(tree);
    }
    tree.children.iter().find_map(|child| find_node(child, id))
}

/// Project one template node; `None` skips whitespace-only text between
/// elements so retained trees stay clean.
fn project_node(
    node: &crate::ir::TemplateNode,
    scope: &EvalScope,
) -> Result<Option<UiNode>, EvalError> {
    match node {
        TemplateNode::Component {
            id,
            kind,
            props,
            children,
            ..
        } => Ok(Some(project_component(id, kind, props, children, scope)?)),
        TemplateNode::Text { id, value, .. } => {
            if value.trim().is_empty() {
                return Ok(None);
            }
            Ok(Some(UiNode {
                id: id.clone(),
                kind: NodeKind::Text,
                props: BTreeMap::from([(
                    "text".to_owned(),
                    serde_json::Value::String(value.clone()),
                )]),
                children: Vec::new(),
            }))
        }
        TemplateNode::Interpolation { id, expression, .. } => {
            let value = evaluate(expression, scope)?;
            Ok(Some(UiNode {
                id: id.clone(),
                kind: NodeKind::Text,
                props: BTreeMap::from([(
                    "text".to_owned(),
                    serde_json::Value::String(as_text(&value)),
                )]),
                children: Vec::new(),
            }))
        }
        TemplateNode::If {
            id,
            condition,
            consequent,
            alternate,
            ..
        } => {
            let branch = if truthy(&evaluate(condition, scope)?) {
                consequent
            } else {
                alternate
            };
            Ok(Some(branch_wrapper(id, project_forest(branch, scope)?)))
        }
        TemplateNode::Each {
            id,
            collection,
            item,
            index,
            key,
            body,
            fallback,
            ..
        } => {
            let items = evaluate(collection, scope)?;
            let StudioValue::Array(elements) = items else {
                return Err(EvalError::rule("each collection is not an array"));
            };
            let mut rendered = Vec::new();
            if elements.is_empty() {
                for child in fallback {
                    if let Some(ui) = project_node(child, scope)? {
                        rendered.push(ui);
                    }
                }
            } else {
                for (position, element) in elements.iter().enumerate() {
                    let mut scoped = scope.clone();
                    scoped.locals.insert(item.clone(), element.clone());
                    scoped.locals.insert(
                        index.clone().unwrap_or_else(|| "index".to_owned()),
                        number(f64::from(u32::try_from(position).unwrap_or(u32::MAX))),
                    );
                    // Row identity suffixes with the evaluated key so rows
                    // stay unique across items (keys are required).
                    let row_key = as_text(&evaluate(key, &scoped)?);
                    for child in body {
                        if let Some(mut ui) = project_node(child, &scoped)? {
                            suffix_row_ids(&mut ui, &row_key);
                            rendered.push(ui);
                        }
                    }
                }
            }
            Ok(Some(UiNode {
                id: format!("{id}#items"),
                kind: NodeKind::Box,
                props: BTreeMap::new(),
                children: rendered,
            }))
        }
    }
}

/// Suffix every id in a projected row subtree with its row key.
///
/// Author ids can never contain `#` (non-canonical ids fail validation),
/// so suffixed ids cannot collide with authored ones.
fn suffix_row_ids(node: &mut UiNode, key: &str) {
    node.id = format!("{}#{key}", node.id);
    for child in &mut node.children {
        suffix_row_ids(child, key);
    }
}

/// Project one component node with evaluated props and children.
///
/// The projection shapes trees to the protocol's cardinality rules so every
/// emitted mount validates: `Text`/`Button` children fold into their content
/// prop (the host renders leaves from props, never children), and
/// single-child containers (`Card` and kin) wrap extra children in an
/// implicit `Column` instead of failing the mount.
fn project_component(
    id: &str,
    kind: &str,
    props: &[crate::ir::PropBinding],
    children: &[crate::ir::TemplateNode],
    scope: &EvalScope,
) -> Result<UiNode, EvalError> {
    let kind = catalog_kind(kind)?;
    let mut ui_props = BTreeMap::new();
    for binding in props {
        let value = match &binding.value {
            PropValue::Literal(literal) => literal.clone(),
            PropValue::Expression(expression) => evaluate(expression, scope)?,
        };
        ui_props.insert(binding.name.clone(), studio_value_json(&value));
    }
    let projected = project_forest(children, scope)?;
    if kind == NodeKind::Text || kind == NodeKind::Button {
        let prop = if kind == NodeKind::Text {
            "text"
        } else {
            "label"
        };
        let mut folded = String::new();
        for child in &projected {
            if child.kind != NodeKind::Text {
                return Err(EvalError::rule(format!(
                    "`{kind:?}` cannot contain a `{child_kind:?}` child; move it out of node `{id}`",
                    child_kind = child.kind
                )));
            }
            folded.push_str(
                child
                    .props
                    .get("text")
                    .and_then(|text| text.as_str())
                    .unwrap_or_default(),
            );
        }
        if !folded.is_empty() {
            ui_props.insert(prop.to_owned(), serde_json::Value::String(folded));
        }
        return Ok(UiNode {
            id: id.to_owned(),
            kind,
            props: ui_props,
            children: Vec::new(),
        });
    }
    let children = if is_single_child_container(kind) && projected.len() > 1 {
        vec![UiNode {
            id: format!("{id}#wrap"),
            kind: NodeKind::Column,
            props: BTreeMap::new(),
            children: projected,
        }]
    } else {
        projected
    };
    Ok(UiNode {
        id: id.to_owned(),
        kind,
        props: ui_props,
        children,
    })
}

/// Single-child containers (`count <= 1` in the protocol): extra children
/// are wrapped, never rejected, so authored trees always mount.
fn is_single_child_container(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::ListTile
            | NodeKind::ScrollView
            | NodeKind::Card
            | NodeKind::Dialog
            | NodeKind::AlertDialog
            | NodeKind::Popover
            | NodeKind::Sheet
            | NodeKind::BottomSheet
            | NodeKind::Drawer
            | NodeKind::Field
            | NodeKind::InputGroup
    )
}

/// Project a forest, skipping whitespace-only text so retained trees stay
/// clean.
fn project_forest(
    nodes: &[crate::ir::TemplateNode],
    scope: &EvalScope,
) -> Result<Vec<UiNode>, EvalError> {
    let mut projected = Vec::new();
    for child in nodes {
        if let Some(ui) = project_node(child, scope)? {
            projected.push(ui);
        }
    }
    Ok(projected)
}

/// Wrapper node giving a taken branch a stable home in the retained tree.
fn branch_wrapper(id: &str, children: Vec<UiNode>) -> UiNode {
    UiNode {
        id: format!("{id}#branch"),
        kind: NodeKind::Box,
        props: BTreeMap::new(),
        children,
    }
}

/// Dispatch one node event: find the handler by node and trigger family,
/// evaluate its payload, and return the typed emission.
///
/// # Errors
///
/// Returns [`EvalError`] when the payload is not a record or an expression
/// has no evaluation rule.
pub fn dispatch_event(
    _component: &ComponentDefinition,
    scope: &EvalScope,
    node_id: &str,
    event: &str,
    handlers: &[EventHandler],
) -> Result<Option<EmittedEvent>, EvalError> {
    let Some(handler) = handlers
        .iter()
        .find(|handler| handler.node.as_deref() == Some(node_id) && handler.event == event)
    else {
        return Ok(None);
    };
    let StudioExpression::Record(entries) = &handler.payload else {
        return Err(EvalError::rule("handler payload is not a record"));
    };
    let mut payload = BTreeMap::new();
    for (key, value) in entries {
        payload.insert(key.clone(), evaluate(value, scope)?);
    }
    Ok(Some(EmittedEvent {
        name: handler.emit.clone(),
        payload,
    }))
}

fn catalog_kind(name: &str) -> Result<NodeKind, EvalError> {
    let converted = catalog_kind_name(name);
    let quoted = format!("\"{converted}\"");
    serde_json::from_str::<NodeKind>(&quoted)
        .map_err(|_| EvalError::rule(format!("unknown catalog kind `{name}`")))
}

fn studio_value_json(value: &StudioValue) -> serde_json::Value {
    match value {
        StudioValue::String(text) => serde_json::Value::String(text.clone()),
        StudioValue::Boolean(value) => serde_json::Value::Bool(*value),
        StudioValue::Number(literal) => {
            // Whole numbers serialize as integers: protocol validators
            // (`columns`, counts) require integer JSON, not `3.0` floats.
            let integer = literal.value >= 0.0
                && literal.value < 9_007_199_254_740_992.0
                && literal.value.fract() == 0.0;
            if integer {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "range and sign checked immediately above"
                )]
                serde_json::Value::from(literal.value as u64)
            } else {
                serde_json::json!(literal.value)
            }
        }
        StudioValue::Array(elements) => {
            serde_json::Value::Array(elements.iter().map(studio_value_json).collect())
        }
        StudioValue::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), studio_value_json(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NumberLiteral, StudioType};

    fn number(value: f64) -> StudioValue {
        StudioValue::Number(NumberLiteral {
            text: value.to_string(),
            value,
        })
    }

    #[test]
    fn arithmetic_follows_javascript_like_semantics() {
        let scope = EvalScope::default();
        let plus = StudioExpression::Binary {
            operator: "+".to_owned(),
            left: Box::new(StudioExpression::Literal(number(1.0))),
            right: Box::new(StudioExpression::Literal(number(2.0))),
        };
        assert_eq!(evaluate(&plus, &scope).unwrap(), number(3.0));
        let concat = StudioExpression::Binary {
            operator: "+".to_owned(),
            left: Box::new(StudioExpression::Literal(StudioValue::String(
                "Add (".to_owned(),
            ))),
            right: Box::new(StudioExpression::Literal(number(2.0))),
        };
        assert_eq!(
            evaluate(&concat, &scope).unwrap(),
            StudioValue::String("Add (2".to_owned())
        );
        assert_eq!(format_money(125.0), "1.25");
        assert_eq!(format_money(5.0), "0.05");
        assert_eq!(format_money(-5.0), "-0.05");
        assert_eq!(format_money(1.9), "0.01");
    }

    #[test]
    fn dotted_reads_walk_records_and_arrays() {
        let scope = EvalScope {
            locals: BTreeMap::from([(
                "item".to_owned(),
                StudioValue::Record(
                    [("name".to_owned(), StudioValue::String("A".to_owned()))]
                        .into_iter()
                        .collect(),
                ),
            )]),
            ..EvalScope::default()
        };
        assert_eq!(
            scope.read("item.name"),
            Some(StudioValue::String("A".to_owned()))
        );
        assert_eq!(scope.read("item.missing"), None);
        assert_eq!(
            StudioValue::Number(NumberLiteral {
                text: "x".to_owned(),
                value: 1.0
            })
            .ty(),
            StudioType::Number
        );
    }
}
