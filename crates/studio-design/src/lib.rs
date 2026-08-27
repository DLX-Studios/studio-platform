//! The host-independent Studio Design authoring domain.
//!
//! This crate is the primary seam for every Designer caller. Callers submit
//! typed command batches and queries through [`DesignerSession`] and receive
//! owned immutable snapshots, receipts, diagnostics, or conflicts. The source
//! model is deliberately independent of GPUI, storage engines, cloud
//! transports, and Runtime UI trees.

pub mod command;
mod engine;
pub mod model;
pub mod persistence;
pub mod projection;
pub mod session;

pub use command::{
    AppliedBatch, Command, CommandBatch, CommandPrecondition, HistoryEntry, ParentPlacement,
};
pub use engine::{DefaultDesignerSession, validate_layout};
pub use model::{
    AccessibilityProperties, AccessibilityRole, Actor, ActorId, ActorKind, Alignment, BindingPath, ColorValue,
    CompositionId, CompositionInput, DeletionTombstone, DesignNode, DesignNodeSource, DesignToken,
    DesignerDiagnostic, DiagnosticSeverity, InputEnvironment, Interaction, InteractionAction,
    InteractionEvent, InteractionId, InteractionSource, LayoutProperties, Length, LengthUnit,
    InvalidIdentity,
    LayoutPosition, LibraryAssetId, NavigationMode, NodeId, NodeParent, OperationId, Paint,
    Placement, ProjectId, PropertyValue, ResponsiveNodeOverride, ResponsiveVariant, ResponsiveVariantId,
    ReusableComposition, RevisionId, RevisionMetadata, RevisionReason,
    STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, SelectionSnapshot, SlotDefinition,
    StudioDesign, StudioDesignSnapshot, StyleProperties, TokenId, TokenKind, TokenValue,
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
pub use session::{
    BatchConflict, CommandOutcome, CommandReceipt, DesignerQuery, DesignerQueryResult,
    DesignerSession, HistoryOperation, HistorySnapshot, SessionContextUpdate, SessionError,
    SessionStateSnapshot, ToolKind,
};
pub use studio_protocol::NodeKind;
