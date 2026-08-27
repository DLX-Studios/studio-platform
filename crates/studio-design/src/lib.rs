//! The host-independent Studio Design authoring domain.
//!
//! This crate is the primary seam for every Designer caller. Callers submit
//! typed command batches and queries through [`DesignerSession`] and receive
//! owned immutable snapshots, receipts, diagnostics, or conflicts. The source
//! model is deliberately independent of GPUI, storage engines, cloud
//! transports, and Runtime UI trees.

pub mod command;
mod engine;
pub mod manipulation;
pub mod model;
pub mod persistence;
pub mod session;

pub use command::{
    AppliedBatch, Command, CommandBatch, CommandPrecondition, HistoryEntry, ParentPlacement,
};
pub use engine::DefaultDesignerSession;
pub use manipulation::{
    CANVAS_RECT_PROPERTY, CanvasAlignment, CanvasDistribution, CanvasGeometry, CanvasPoint,
    CanvasRect, CanvasSize, GestureContext, GuideAxis, GuideKind, HierarchyEdit, HierarchyNode,
    HierarchySnapshot, HitTestEntry, HitTestIndex, ManipulationError, ResizeHandle, SnapConfig,
    SnapGuide, SnapResult, align_batch, alignment_targets, delete_batch, distribute_batch,
    distribution_targets, drag_batch, duplicate_batch, hierarchy_edit_batch, keyboard_resize_batch,
    nudge_batch, reorder_batch, reparent_batch, resize_batch, resize_rect, restore_batch,
};
pub use model::{
    AccessibilityProperties, AccessibilityRole, Actor, ActorId, ActorKind, BindingPath, ColorValue,
    CompositionId, CompositionInput, DeletionTombstone, DesignNode, DesignNodeSource, DesignToken,
    DesignerDiagnostic, DiagnosticSeverity, Interaction, InteractionAction, InteractionEvent,
    InteractionId, InteractionSource, LayoutProperties, Length, LengthUnit, LibraryAssetId,
    NavigationMode, NodeId, NodeParent, OperationId, Paint, ProjectId, PropertyValue,
    ResponsiveNodeOverride, ResponsiveVariant, ResponsiveVariantId, ReusableComposition,
    RevisionId, RevisionMetadata, RevisionReason, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId,
    SelectionSnapshot, SlotDefinition, StudioDesign, StudioDesignSnapshot, StyleProperties,
    TokenId, TokenKind, TokenValue, TombstoneReference, UndoGroupId, ValueKind,
};
pub use persistence::{
    DesignerPersistence, DesignerTransaction, DurableDesignerState, InMemoryDesignerPersistence,
    PersistenceError, PersistenceErrorCode, SessionFuture,
};
pub use session::{
    BatchConflict, CommandOutcome, CommandReceipt, DesignerQuery, DesignerQueryResult,
    DesignerSession, HistoryOperation, HistorySnapshot, SessionContextUpdate, SessionError,
    SessionStateSnapshot, ToolKind,
};
pub use studio_protocol::NodeKind;
