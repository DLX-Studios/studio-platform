//! Closed, versioned Studio Design source-model schemas.
#![allow(
    missing_docs,
    reason = "closed serde record fields mirror their documented domain type"
)]

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use studio_protocol::NodeKind;
use thiserror::Error;

/// The only Studio Design schema version accepted by this crate.
pub const STUDIO_DESIGN_SCHEMA_VERSION: u16 = 1;

/// A stable identifier failed the bounded opaque-identity contract.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("identity must be 1..=128 bytes and contain no control characters")]
pub struct InvalidIdentity;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a bounded opaque identity.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidIdentity`] for empty, oversized, or control-bearing values.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentity> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > 128
                    || value.chars().any(char::is_control)
                {
                    return Err(InvalidIdentity);
                }
                Ok(Self(value))
            }

            /// Borrow the opaque wire value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

define_id!(/// Stable Studio Project identity.
    ProjectId);
define_id!(/// Stable visual-node identity.
    NodeId);
define_id!(/// Stable screen identity.
    ScreenId);
define_id!(/// Stable Reusable Composition identity.
    CompositionId);
define_id!(/// Stable design-token identity.
    TokenId);
define_id!(/// Stable responsive-variant identity.
    ResponsiveVariantId);
define_id!(/// Stable device-profile identity used by the Designer preview.
    DeviceProfileId);
define_id!(/// Stable interaction identity.
    InteractionId);
define_id!(/// Stable admitted Library asset identity.
    LibraryAssetId);
define_id!(/// Stable actor identity.
    ActorId);
define_id!(/// Stable command-operation identity.
    OperationId);
define_id!(/// Stable named undo-group identity.
    UndoGroupId);

/// Monotonic immutable revision identity within one project.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RevisionId(u64);

impl RevisionId {
    /// The initial persisted project revision.
    pub const INITIAL: Self = Self(0);

    /// Construct a revision identity from its durable sequence.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the durable sequence.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Return the next sequence, or `None` if the sequence is exhausted.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// The authority that submitted an operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Human,
    Agent,
    Mcp,
    Ingestion,
    Extension,
    System,
}

/// Safe actor provenance recorded on every revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
    pub display_name: String,
}

/// The complete Studio Design source document at one revision.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudioDesign {
    pub schema_version: u16,
    pub project_id: ProjectId,
    pub name: String,
    pub nodes: BTreeMap<NodeId, DesignNode>,
    pub parents: BTreeMap<NodeId, NodeParent>,
    pub screens: BTreeMap<ScreenId, Screen>,
    pub screen_order: Vec<ScreenId>,
    pub compositions: BTreeMap<CompositionId, ReusableComposition>,
    pub tokens: BTreeMap<TokenId, DesignToken>,
    pub responsive_variants: BTreeMap<ResponsiveVariantId, ResponsiveVariant>,
    pub interactions: BTreeMap<InteractionId, Interaction>,
}

impl StudioDesign {
    /// Create an empty version-1 source document.
    #[must_use]
    pub fn empty(project_id: ProjectId, name: impl Into<String>) -> Self {
        Self {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            project_id,
            name: name.into(),
            nodes: BTreeMap::new(),
            parents: BTreeMap::new(),
            screens: BTreeMap::new(),
            screen_order: Vec::new(),
            compositions: BTreeMap::new(),
            tokens: BTreeMap::new(),
            responsive_variants: BTreeMap::new(),
            interactions: BTreeMap::new(),
        }
    }
}

/// One entry in the global flat stable-ID node map.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignNode {
    pub schema_version: u16,
    pub id: NodeId,
    pub name: String,
    pub source: DesignNodeSource,
    pub children: Vec<NodeId>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub layout: LayoutProperties,
    pub style: StyleProperties,
    pub accessibility: AccessibilityProperties,
    pub responsive_overrides: BTreeMap<ResponsiveVariantId, ResponsiveNodeOverride>,
    pub interaction_ids: Vec<InteractionId>,
}

impl DesignNode {
    /// Create a version-1 primitive with no children or authored properties.
    #[must_use]
    pub fn primitive(id: NodeId, name: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id,
            name: name.into(),
            source: DesignNodeSource::Primitive { kind },
            children: Vec::new(),
            properties: BTreeMap::new(),
            layout: LayoutProperties::default(),
            style: StyleProperties::default(),
            accessibility: AccessibilityProperties::default(),
            responsive_overrides: BTreeMap::new(),
            interaction_ids: Vec::new(),
        }
    }
}

/// Whether a node is a catalog primitive or a project-owned composition instance.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignNodeSource {
    Primitive {
        kind: NodeKind,
    },
    CompositionInstance {
        composition_id: CompositionId,
        definition_version: u32,
        inputs: BTreeMap<String, PropertyValue>,
        admitted_overrides: BTreeMap<String, PropertyValue>,
    },
}

/// The validated owner of one node in the flat parent index.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeParent {
    Screen { screen_id: ScreenId },
    Node { node_id: NodeId },
    Composition { composition_id: CompositionId },
}

/// One routable screen whose root is stored in the global node map.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub schema_version: u16,
    pub id: ScreenId,
    pub name: String,
    pub route: String,
    pub root_node_id: NodeId,
}

/// One project-owned Reusable Composition definition.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReusableComposition {
    pub schema_version: u16,
    pub id: CompositionId,
    pub name: String,
    pub definition_version: u32,
    pub root_node_id: NodeId,
    pub inputs: BTreeMap<String, CompositionInput>,
    pub slots: BTreeMap<String, SlotDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionInput {
    pub value_kind: ValueKind,
    pub required: bool,
    pub default: Option<PropertyValue>,
    pub overridable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SlotDefinition {
    pub required: bool,
    pub accepts_multiple: bool,
}

/// Closed property-value categories used by composition contracts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    String,
    Boolean,
    Integer,
    Decimal,
    Length,
    Color,
    Token,
    Binding,
    Node,
    Asset,
    List,
}

/// One closed typed authored property value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PropertyValue {
    String(String),
    Boolean(bool),
    Integer(i64),
    Decimal(String),
    Length(Length),
    Color(ColorValue),
    Token(TokenId),
    Binding(BindingPath),
    Node(NodeId),
    Asset(LibraryAssetId),
    List(Vec<Self>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BindingPath {
    pub collection: String,
    pub segments: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Length {
    pub value: String,
    pub unit: LengthUnit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Pixels,
    Percent,
    ViewportWidth,
    ViewportHeight,
    Rem,
    Auto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "space",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ColorValue {
    SrgbHex(String),
    Semantic(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Paint {
    Color(ColorValue),
    Token(TokenId),
    None,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutProperties {
    pub schema_version: u16,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub gap: Option<Length>,
    pub padding: Option<Length>,
    pub placement: Option<Placement>,
    pub alignment: Option<Alignment>,
}

impl Default for LayoutProperties {
    fn default() -> Self {
        Self {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            width: None,
            height: None,
            gap: None,
            padding: None,
            placement: None,
            alignment: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Flow,
    Overlay,
    Absolute,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Start,
    Center,
    End,
    Stretch,
    SpaceBetween,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StyleProperties {
    pub schema_version: u16,
    pub background: Option<Paint>,
    pub foreground: Option<Paint>,
    pub opacity: Option<String>,
    pub corner_radius: Option<Length>,
    pub border_width: Option<Length>,
    pub border_color: Option<Paint>,
}

impl Default for StyleProperties {
    fn default() -> Self {
        Self {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            background: None,
            foreground: None,
            opacity: None,
            corner_radius: None,
            border_width: None,
            border_color: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilityProperties {
    pub schema_version: u16,
    pub label: Option<String>,
    pub hint: Option<String>,
    pub role: Option<AccessibilityRole>,
    pub hidden: bool,
}

impl Default for AccessibilityProperties {
    fn default() -> Self {
        Self {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            label: None,
            hint: None,
            role: None,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilityRole {
    Button,
    Checkbox,
    Heading,
    Image,
    Link,
    List,
    ListItem,
    Navigation,
    Text,
    TextInput,
}

/// One named base-plus-breakpoint profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsiveVariant {
    pub schema_version: u16,
    pub id: ResponsiveVariantId,
    pub name: String,
    pub minimum_width: Option<u32>,
    pub maximum_width: Option<u32>,
    pub input: InputEnvironment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputEnvironment {
    Any,
    Pointer,
    Touch,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsiveNodeOverride {
    pub schema_version: u16,
    pub properties: BTreeMap<String, PropertyValue>,
    pub layout: LayoutProperties,
    pub style: StyleProperties,
}

/// One versioned project-owned design token.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignToken {
    pub schema_version: u16,
    pub id: TokenId,
    pub name: String,
    pub kind: TokenKind,
    pub value: TokenValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Color,
    Length,
    Number,
    String,
    Typography,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TokenValue {
    Color(ColorValue),
    Length(Length),
    Number(String),
    String(String),
    Typography(TypographyToken),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyToken {
    pub family: String,
    pub weight: u16,
    pub size: String,
    pub line_height: String,
}

/// A typed declarative interaction reference graph entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub schema_version: u16,
    pub id: InteractionId,
    pub source: InteractionSource,
    pub action: InteractionAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionSource {
    pub node_id: NodeId,
    pub event: InteractionEvent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEvent {
    Pressed,
    Changed,
    Submitted,
    Focused,
    Blurred,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum InteractionAction {
    Navigate {
        screen_id: ScreenId,
        mode: NavigationMode,
    },
    SetProperty {
        node_id: NodeId,
        property: String,
        value: PropertyValue,
    },
    ToggleVisibility {
        node_id: NodeId,
    },
    Emit {
        event: String,
    },
    Sequence {
        interaction_ids: Vec<InteractionId>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationMode {
    Push,
    Replace,
    Reset,
    PopTo,
}

/// Metadata describing why one immutable revision exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionMetadata {
    pub id: RevisionId,
    pub parent_id: Option<RevisionId>,
    pub operation_id: OperationId,
    pub actor: Actor,
    pub undo_group_id: UndoGroupId,
    pub undo_group_name: String,
    pub reason: RevisionReason,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionReason {
    Initial,
    Command,
    Undo,
    Redo,
}

/// An owned snapshot that cannot mutate session state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudioDesignSnapshot {
    pub revision: RevisionMetadata,
    pub design: StudioDesign,
    pub tombstones: BTreeMap<NodeId, DeletionTombstone>,
}

/// A deleted subtree plus placement and reference context sufficient for restore.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionTombstone {
    pub schema_version: u16,
    pub root_node_id: NodeId,
    pub nodes: BTreeMap<NodeId, DesignNode>,
    pub parents: BTreeMap<NodeId, NodeParent>,
    pub detached_from: NodeParent,
    pub detached_index: usize,
    pub deleted_in_revision: Option<RevisionId>,
    pub references: Vec<TombstoneReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TombstoneReference {
    pub owner: String,
    pub field: String,
    pub target_node_id: NodeId,
}

/// Session-owned node selection returned as an immutable value.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub node_ids: Vec<NodeId>,
    pub primary: Option<NodeId>,
}

/// Severity of a safe authoring diagnostic.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Stable, safe diagnostic returned by the Designer seam.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignerDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub node_id: Option<NodeId>,
    pub interaction_id: Option<InteractionId>,
}
