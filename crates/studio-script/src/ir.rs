//! The typed Studio intermediate representation.
//!
//! This module owns the closed, versioned IR that both Designer-authored
//! projects and hand-authored Studio Script compile through.  The IR mirrors
//! the parser-of-record semantic model for screens and nodes, adds declarative
//! navigation behaviors, and is the single input of the `AssemblyScript`
//! lowering backend.
//!
//! The v1 skeleton subset covers static screen trees and typed navigation
//! actions.  Dynamic constructs such as `$item.*` bindings carry no IR node;
//! they are rejected with stable source-linked diagnostics during lowering
//! instead of being carried through for a later runtime to interpret.

use crate::Span;
use crate::types::{StudioType, StudioValue};

/// The only Studio IR version currently produced by this crate.
pub const STUDIO_IR_VERSION: u16 = 2;

/// A lowered Studio module: screens plus declarative navigation behaviors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioIrModule {
    /// Closed IR version.
    pub version: u16,
    /// Screens in authored document order.
    pub screens: Vec<IrScreen>,
    /// Navigation actions in behavior-script order.
    pub actions: Vec<IrNavigationAction>,
}

/// One mounted screen with a stable identity and a derived v1 route.
///
/// The route is always `/<screen-id>` in the v1 skeleton; nested route trees
/// arrive with the projection seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrScreen {
    /// Stable screen identity copied from the root element id.
    pub id: String,
    /// Derived v1 route of the screen.
    pub route: String,
    /// Static root node of the screen tree.
    pub root: IrNode,
}

/// A static node of a screen tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrNode {
    /// A catalog element.
    Element(IrElement),
    /// A text leaf with a derived stable identity.
    Text(IrText),
}

/// A catalog element mirrored from the parser semantic model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrElement {
    /// Stable node identity.
    pub id: String,
    /// Catalog kind in canonical lowercase protocol form.
    pub kind: String,
    /// Static properties in deterministic key order.
    pub properties: Vec<(String, IrProperty)>,
    /// Ordered nested content.
    pub children: Vec<IrNode>,
    /// Best-effort source span of the element identity.
    pub span: Span,
}

/// A text leaf.  Parser text nodes have no identity, so lowering derives one
/// deterministically as `<parent-id>-text-<ordinal>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrText {
    /// Derived stable identity.
    pub id: String,
    /// Normalized text content.
    pub text: String,
    /// Best-effort source span of the text content.
    pub span: Span,
}

/// A static property value admitted by the v1 skeleton subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrProperty {
    /// A quoted UTF-8 string.
    String(String),
    /// A boolean literal.
    Boolean(bool),
    /// A number retained in canonical lexical form.
    Number(String),
}

/// A declarative navigation behavior bound to one node trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrNavigationAction {
    /// Stable identity of the triggering node.
    pub trigger_node_id: String,
    /// Trigger event on that node.
    pub trigger_event: IrTriggerEvent,
    /// Navigation operation issued to the host.
    pub operation: IrNavigationOperation,
    /// Source span of the behavior statement.
    pub span: Span,
}

/// The trigger events admitted by the v1 behavior grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrTriggerEvent {
    /// Primary press activation.
    Pressed,
    /// Value change activation.
    Changed,
    /// Submit activation.
    Submitted,
}

impl IrTriggerEvent {
    /// The canonical event string used in the wire protocol.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pressed => "pressed",
            Self::Changed => "changed",
            Self::Submitted => "submitted",
        }
    }

    /// Parse a trigger-event keyword of the behavior grammar.
    #[must_use]
    pub fn parse(keyword: &str) -> Option<Self> {
        match keyword {
            "pressed" => Some(Self::Pressed),
            "changed" => Some(Self::Changed),
            "submitted" => Some(Self::Submitted),
            _ => None,
        }
    }
}

/// A closed navigation operation mirroring the v1 protocol commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrNavigationOperation {
    /// Push a route onto the stack.
    Push {
        /// Destination route.
        route: String,
    },
    /// Replace the top of the stack.
    Replace {
        /// Destination route.
        route: String,
    },
    /// Pop to the nearest occurrence of a route.
    PopTo {
        /// Destination route.
        route: String,
    },
    /// Reset the stack to a single route.
    Reset {
        /// Destination route.
        route: String,
    },
    /// Pop the top of the stack.
    Pop,
}

impl IrNavigationOperation {
    /// The destination route, if the operation carries one.
    #[must_use]
    pub fn route(&self) -> Option<&str> {
        match self {
            Self::Push { route }
            | Self::Replace { route }
            | Self::PopTo { route }
            | Self::Reset { route } => Some(route),
            Self::Pop => None,
        }
    }
}

/// One compiled `.studio` module: the v2 dynamic model.
///
/// Stable IDs derive from module identity and structural/source identity, so
/// repeated compilations of identical sources produce byte-identical modules.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StudioModule {
    /// Closed IR version.
    pub version: u16,
    /// Stable module identity (source path role plus content fingerprint).
    pub id: String,
    /// Static imports in source order.
    pub imports: Vec<ModuleImport>,
    /// Exported component names in source order.
    pub exports: Vec<String>,
    /// Component definitions in source order.
    pub components: Vec<ComponentDefinition>,
    /// Module identities this module depends on, sorted.
    pub dependencies: Vec<String>,
}

/// One static import of another module or virtual contract.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModuleImport {
    /// Imported module identity or virtual contract (for example
    /// `@studio/generated/routes`).
    pub module: String,
    /// Imported names.
    pub names: Vec<String>,
    /// Source span of the import statement.
    pub span: Span,
}

/// One component definition with typed interface and template.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComponentDefinition {
    /// Stable component identity (module identity plus declaration site).
    pub id: String,
    /// Component name as declared.
    pub name: String,
    /// Typed props in declaration order.
    pub props: Vec<PropDefinition>,
    /// `$state` slots in declaration order.
    pub state: Vec<StateSlot>,
    /// `$derived` slots in declaration order.
    pub derived: Vec<DerivedSlot>,
    /// Typed event handlers in source order.
    pub handlers: Vec<EventHandler>,
    /// Folded content bindings: `(node id, content prop, value expression)`
    /// for `Text`/`Button` nodes, recorded once at lowering so the evaluator
    /// and the `AssemblyScript` backend project byte-identical trees.
    pub content: Vec<ContentBinding>,
    /// Root template forest.
    pub template: Vec<TemplateNode>,
    /// Source span of the declaration.
    pub span: Span,
}

/// One typed component prop.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropDefinition {
    /// Prop name.
    pub name: String,
    /// Closed type.
    pub ty: StudioType,
    /// Whether callers must supply it.
    pub required: bool,
    /// Default value for optional props.
    pub default: Option<StudioValue>,
    /// Source span of the declaration.
    pub span: Span,
}

/// One `$state` slot: named, typed reactive storage.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateSlot {
    /// Stable slot identity.
    pub id: String,
    /// Binding name.
    pub name: String,
    /// Closed type.
    pub ty: StudioType,
    /// Initializer expression snapshot.
    pub initial: StudioExpression,
    /// Source span of the declaration.
    pub span: Span,
}

/// One `$derived` slot: recomputed from its dependencies.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DerivedSlot {
    /// Stable slot identity.
    pub id: String,
    /// Binding name.
    pub name: String,
    /// Closed type.
    pub ty: StudioType,
    /// Derivation expression.
    pub expression: StudioExpression,
    /// Slot names this derivation reads.
    pub dependencies: Vec<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// One typed event handler: a node event bound to state mutations and an
/// optional emission.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventHandler {
    /// Stable handler identity.
    pub id: String,
    /// Node that raises the event, if bound to a template node.
    pub node: Option<String>,
    /// Event name (for example `onclick`).
    pub event: String,
    /// Emitted event name (empty for pure state handlers).
    pub emit: String,
    /// Emission payload expression.
    pub payload: StudioExpression,
    /// State mutations applied before the emission, in source order.
    pub mutations: Vec<Mutation>,
    /// Source span of the handler.
    pub span: Span,
}

/// One state-slot mutation inside a handler body.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mutation {
    /// Target `$state` slot name.
    pub slot: String,
    /// Mutation operation.
    pub op: MutationOp,
    /// Operand expression (literal zero for increment/decrement).
    pub operand: StudioExpression,
    /// Source span of the statement.
    pub span: Span,
}

/// Supported state-mutation operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// A template node of a component: closed tree with stable identities.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TemplateNode {
    /// A catalog component instance.
    Component {
        /// Stable node identity.
        id: String,
        /// Catalog kind (for example `Card`).
        kind: String,
        /// Explicit key expression, if declared.
        explicit_key: Option<StudioExpression>,
        /// Bound properties in source order.
        props: Vec<PropBinding>,
        /// Ordered nested content.
        children: Vec<TemplateNode>,
        /// Source span of the element.
        span: Span,
    },
    /// Literal text.
    Text {
        /// Stable node identity.
        id: String,
        /// Text content.
        value: String,
        /// Source span of the text.
        span: Span,
    },
    /// `{expression}` interpolation.
    Interpolation {
        /// Stable node identity.
        id: String,
        /// Interpolated expression.
        expression: StudioExpression,
        /// Source span of the interpolation.
        span: Span,
    },
    /// `{#if}` conditional.
    If {
        /// Stable node identity.
        id: String,
        /// Condition expression.
        condition: StudioExpression,
        /// Nodes rendered when the condition holds.
        consequent: Vec<TemplateNode>,
        /// Nodes rendered otherwise.
        alternate: Vec<TemplateNode>,
        /// Source span of the block.
        span: Span,
    },
    /// Keyed `{#each}` iteration.
    Each {
        /// Stable node identity.
        id: String,
        /// Iterated collection expression.
        collection: StudioExpression,
        /// Item binding name.
        item: String,
        /// Index binding name, if declared.
        index: Option<String>,
        /// Key expression (required by validation).
        key: StudioExpression,
        /// Nodes rendered per item.
        body: Vec<TemplateNode>,
        /// Nodes rendered for an empty collection.
        fallback: Vec<TemplateNode>,
        /// Source span of the block.
        span: Span,
    },
}

/// One bound component property.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropBinding {
    /// Property name.
    pub name: String,
    /// Bound value: literal or expression.
    pub value: PropValue,
    /// Source span of the attribute.
    pub span: Span,
}

/// One folded content binding: the value expression for a leaf content
/// prop, shared by the evaluator and the `AssemblyScript` backend.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContentBinding {
    /// Node carrying the content.
    pub node_id: String,
    /// Content prop (`text` on `Text`, `label` on `Button`).
    pub prop: String,
    /// Value expression over component bindings.
    pub expression: StudioExpression,
}

/// A component property value.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PropValue {
    /// Static literal.
    Literal(StudioValue),
    /// Dynamic expression evaluated per render.
    Expression(StudioExpression),
}

/// A closed, typed Studio expression. Raw source text is never executable.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StudioExpression {
    /// Static literal.
    Literal(StudioValue),
    /// Read a component prop.
    ReadProp(String),
    /// Read a state slot.
    ReadState(String),
    /// Read a derived slot.
    ReadDerived(String),
    /// Read an iteration or handler local.
    ReadLocal(String),
    /// Unary operation.
    Unary {
        /// Operator spelling (for example `!` or `-`).
        operator: String,
        /// Operand.
        operand: Box<StudioExpression>,
    },
    /// Binary operation.
    Binary {
        /// Operator spelling (for example `+` or `===`).
        operator: String,
        /// Left operand.
        left: Box<StudioExpression>,
        /// Right operand.
        right: Box<StudioExpression>,
    },
    /// Conditional expression.
    Conditional {
        /// Condition.
        condition: Box<StudioExpression>,
        /// Value when the condition holds.
        consequent: Box<StudioExpression>,
        /// Value otherwise.
        alternate: Box<StudioExpression>,
    },
    /// Array literal.
    Array(Vec<StudioExpression>),
    /// Record literal with named fields.
    Record(Vec<(String, StudioExpression)>),
    /// Call to an explicitly approved pure function.
    CallApprovedFunction {
        /// Approved function identity.
        function: String,
        /// Argument expressions.
        arguments: Vec<StudioExpression>,
    },
}
