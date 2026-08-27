//! The host-independent Studio Design authoring domain.
//!
//! This crate is the primary seam for every Designer caller. Callers submit
//! typed command batches and queries through [`DesignerSession`] and receive
//! owned immutable snapshots, receipts, diagnostics, or conflicts. The source
//! model is deliberately independent of GPUI, storage engines, cloud
//! transports, and Runtime UI trees.

pub mod agent;
pub mod command;
pub mod content;
mod engine;
pub mod model;
pub mod persistence;
pub mod projection;
pub mod responsive;
pub mod session;
pub mod workspace;

pub use agent::{
    AgentBatch, AgentBatchOutcome, AgentBatchResult, AgentCancellation, AgentChannel,
    AgentCheckFeedback, AgentChecker, AgentCommandBatch, AgentCommandSchema, AgentConflict,
    AgentEvent, AgentEventSink, AgentProgress, AgentProjectSummary, AgentReadError,
    AgentReadResult, AgentReadScope, AgentRunError, AgentRunId, AgentRunRequest, AgentRunSummary,
    AgentSchemaSnapshot, AgentScope, AgentSubtreeSnapshot, InvalidAgentRunId, LiveAgentChannel,
    NoopAgentChecker, NoopAgentEventSink,
};
pub use command::{
    AppliedBatch, Command, CommandBatch, CommandPrecondition, HistoryEntry, ParentPlacement,
};
pub use content::*;
pub use engine::{DefaultDesignerSession, validate_layout};
pub use model::{
    AccessibilityProperties, AccessibilityRole, Actor, ActorId, ActorKind, Alignment, BindingId,
    BindingPath, BindingSource, BorderToken, CollectionId, CollectionPreview, ColorValue,
    CompositionId, CompositionInput, ContentBinding, ContentCollection, ContentCollectionSchema,
    ContentFieldKind, ContentFieldSchema, ContentFixture, ContentRecord, DeletionTombstone,
    DesignNode, DesignNodeSource, DesignToken, DesignerDiagnostic, DeviceProfileId,
    DiagnosticSeverity, FixtureKind, FormDefinition, FormFieldSchema, FormId,
    FormValidationResult, InputEnvironment, InspectedTokenValue, Interaction, InteractionAction,
    InteractionEvent, InteractionId, InteractionSource, LayoutProperties, Length, LengthUnit,
    InvalidIdentity,
    LayoutPosition, LibraryAssetId, NavigationMode, NodeId, NodeParent, OperationId, Paint,
    Placement, ProjectId, PropertyValue, RecordId, ResponsiveNodeOverride, ResponsiveVariant,
    ResponsiveVariantId,
    ReusableComposition, RevisionId, RevisionMetadata, RevisionReason,
    STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, SelectionSnapshot, SlotDefinition,
    StudioDesign, StudioDesignSnapshot, StyleProperties, TokenId, TokenKind, TokenOverride,
    TokenUsage, TokenValue, TombstoneReference, TypographyToken, UndoGroupId, ValueKind,
};
pub use persistence::{
    DesignerPersistence, DesignerTransaction, DurableDesignerState, InMemoryDesignerPersistence,
    PersistenceError, PersistenceErrorCode, SessionFuture,
};
pub use projection::{
    CODE_ASSET_MISSING, CODE_NODE_INVALID, CODE_PROPERTY_INVALID, CODE_PROTOCOL_INVALID,
    CODE_SCREEN_INVALID, CODE_UNSUPPORTED, LibraryAsset, LibrarySnapshot, ProjectionDiagnostic,
    ProjectionError, ProjectionOptions, ProjectionReport, RuntimeProjection, project_report,
    project_runtime,
};
pub use responsive::{
    BreakpointProvenance, CompareReport, DeviceInput, DeviceProfile, DeviceProfileMatrix, Insets,
    Orientation, ProfileDifference, PropertyPath, PropertyProvenance, ResolvedNode, ResolvedValue,
    ResponsiveValue, Viewport, compare_profiles, inspect_node, resolve_node, select_variant,
};
pub use session::{
    AgentRun, AgentRunStatus, BatchConflict, CanvasStateSnapshot, CanvasTransform, CommandOutcome,
    CommandReceipt, DesignerQuery, DesignerQueryResult, DesignerSession, HistoryOperation,
    HistorySnapshot, SessionContextUpdate, SessionError, SessionStateSnapshot, ToolKind,
    UnsavedWork,
};
pub use studio_protocol::NodeKind;
pub use workspace::{
    CommandDescriptor, EditorView, InMemoryWorkspacePersistence, PanelArrangement, PanelGeometry,
    PanelId, PanelState, ViewSwitchSnapshot, WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceCommand,
    WorkspaceController, WorkspaceError, WorkspacePersistence, WorkspaceRecord, WorkspaceState,
    command_registry, find_commands,
};
