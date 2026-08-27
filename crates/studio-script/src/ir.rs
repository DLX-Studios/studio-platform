//! The typed Studio intermediate representation.
//!
//! This module owns the closed, versioned IR that both Designer-authored
//! projects and hand-authored Studio Script compile through.  The IR mirrors
//! the parser-of-record semantic model for screens and nodes, adds declarative
//! navigation behaviors, and is the single input of the AssemblyScript
//! lowering backend.
//!
//! The v1 skeleton subset covers static screen trees and typed navigation
//! actions.  Dynamic constructs such as `$item.*` bindings carry no IR node;
//! they are rejected with stable source-linked diagnostics during lowering
//! instead of being carried through for a later runtime to interpret.

use crate::Span;

/// The only Studio IR version currently produced by this crate.
pub const STUDIO_IR_VERSION: u16 = 1;

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
