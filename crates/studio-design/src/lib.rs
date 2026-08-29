//! The host-independent Studio Design authoring domain.
//!
//! This crate is the primary seam for every Designer caller. Callers submit
//! typed command batches and queries through [`DesignerSession`] and receive
//! owned immutable snapshots, receipts, diagnostics, or conflicts. The source
//! model is deliberately independent of GPUI, storage engines, cloud
//! transports, and Runtime UI trees.

pub mod access;
pub mod agent;
pub mod agent_conversation;
pub mod command;
pub mod content;
pub mod content_adapter;
mod engine;
pub mod library_adapter;
pub mod manipulation;
pub mod mcp;
pub mod model;
pub mod navigation;
pub mod persistence;
pub mod projection;
pub mod prototype;
pub mod recovery;
pub mod responsive;
pub mod script_editor;
pub mod session;
pub mod ux;
pub mod workspace;

pub use access::{
    DesignerCapability, DesignerScope, ScopeDenied, ScopedDesignerAccess, ScopedDesignerSession,
    ScopedOperation,
};
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
pub use manipulation::{
    CANVAS_RECT_PROPERTY, CanvasAlignment, CanvasDistribution, CanvasGeometry, CanvasPoint,
    CanvasRect, CanvasSize, GestureContext, GuideAxis, GuideKind, HierarchyEdit, HierarchyNode,
    HierarchySnapshot, HitTestEntry, HitTestIndex, ManipulationError, ResizeHandle, SnapConfig,
    SnapGuide, SnapResult, align_batch, alignment_targets, delete_batch, distribute_batch,
    distribution_targets, drag_batch, duplicate_batch, hierarchy_edit_batch, keyboard_resize_batch,
    nudge_batch, reorder_batch, reparent_batch, resize_batch, resize_rect, restore_batch,
};
pub use mcp::{McpClient, McpClientError};
pub use model::{
    AccessibilityProperties, AccessibilityRole, Actor, ActorId, ActorKind, Alignment, BindingId,
    BindingPath, BindingSource, BorderToken, CollectionId, CollectionPreview, ColorValue,
    CompositionId, CompositionInput, ContentBinding, ContentCollection, ContentCollectionSchema,
    ContentFieldKind, ContentFieldSchema, ContentFixture, ContentRecord, DeletionTombstone,
    DesignNode, DesignNodeSource, DesignToken, DesignerDiagnostic, DeviceProfileId,
    DiagnosticSeverity, FixtureKind, FormDefinition, FormFieldSchema, FormId, FormValidationResult,
    InputEnvironment, InspectedTokenValue, InstalledPlugin, Interaction, InteractionAction,
    InteractionEvent, InteractionId, InteractionSource, InvalidIdentity, LayoutPosition,
    LayoutProperties, Length, LengthUnit, LibraryAssetId, NavigationMode, NodeId, NodeParent,
    OperationId, Paint, Placement, PluginId, ProjectId, PropertyValue, RecordId,
    ResponsiveNodeOverride, ResponsiveVariant, ResponsiveVariantId, ReusableComposition,
    RevisionId, RevisionMetadata, RevisionReason, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId,
    SelectionSnapshot, SettingKey, SettingValue, SlotDefinition, SourceProvenance, StudioDesign,
    StudioDesignSnapshot, StyleProperties, TokenId, TokenKind, TokenOverride, TokenUsage,
    TokenValue, TombstoneReference, TypographyToken, UndoGroupId, ValueKind,
};
pub use navigation::{
    CODE_INTERACTION_CYCLE, CODE_INTERACTION_SOURCE_MISSING, CODE_INTERACTION_TARGET_MISSING,
    CODE_ROUTE_DUPLICATE, CODE_ROUTE_INVALID, EventInspectorEntry, InteractionGraph,
    InteractionInspectorEntry, NavigationEdge, NavigationGraph, NavigationScreen,
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
pub use prototype::{
    PrototypeDispatch, PrototypeEffect, PrototypeError, PrototypeEvent, PrototypeSession,
    PrototypeStateSnapshot, PrototypeTraceEntry,
};
pub use recovery::{
    ConflictCenter, ConflictIntent, ConflictPersistence, ConflictRecord, ConflictStatus,
    InMemoryConflictPersistence, InMemoryRecoveryPersistence, JournalEntry, LogicalSnapshot,
    RESILIENCE_SCHEMA_VERSION, RecoveryBundle, RecoveryCenter, RecoveryPersistence, RecoveryRecord,
    RecoveryState, ResilienceError, ResolutionChoice, ResolutionPlan,
};
pub use responsive::{
    BreakpointProvenance, CompareReport, DeviceInput, DeviceProfile, DeviceProfileMatrix, Insets,
    Orientation, ProfileDifference, PropertyPath, PropertyProvenance, ResolvedNode, ResolvedValue,
    ResponsiveValue, Viewport, compare_profiles, inspect_node, resolve_node, select_variant,
};
pub use script_editor::{
    EditorSnapshot, OutlineNode, ScriptCommitMetadata, ScriptCommitOutcome, ScriptCommitPlan,
    ScriptDiagnostic, ScriptDocumentAdapter, ScriptEdit, ScriptEditor, ScriptEditorError,
    SyntaxToken, SyntaxTokenKind,
};
pub use session::{
    AgentRun, AgentRunStatus, BatchConflict, CanvasStateSnapshot, CanvasTransform, CommandOutcome,
    CommandReceipt, DesignerQuery, DesignerQueryResult, DesignerSession, HistoryOperation,
    HistorySnapshot, SessionContextUpdate, SessionError, SessionStateSnapshot, ToolKind,
    UnsavedWork,
};
pub use studio_protocol::NodeKind;
pub use ux::{
    BrandSlot, GeneratedSettingsSurface, ImportDestination, ImportProposal, ImportReview,
    ImportReviewError, ImportReviewStatus, ImportSource, ImportWarning, InferredEntity,
    PluginBrowseCard, PluginCatalog, SettingsControl, SettingsError, SettingsFieldView,
    SettingsTab, TemplateDefinition, TemplateError, TemplateNode, TemplateScreen,
    plugin_install_batch, setting_change_batch,
};
pub use workspace::{
    CommandDescriptor, EditorView, InMemoryWorkspacePersistence, PanelArrangement, PanelGeometry,
    PanelId, PanelState, ViewSwitchSnapshot, WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceCommand,
    WorkspaceController, WorkspaceError, WorkspacePersistence, WorkspaceRecord, WorkspaceState,
    command_registry, find_commands,
};
