//! Host-independent state for the Studio Agent conversation surface.
//!
//! The conversation is deliberately modeled without GPUI handles.  A native view can
//! project this state into Focus or Workbench while the state itself remains serializable,
//! testable, and portable to the channel/session adapter used by the host.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const MAX_ID_LENGTH: usize = 128;
const MAX_TEXT_LENGTH: usize = 32_768;

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn valid_content(value: &str, max: usize) -> bool {
    value.len() <= max
        && value
            .chars()
            .all(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ConversationInputError> {
                let value = value.into();
                if !valid_text(&value, MAX_ID_LENGTH) {
                    return Err(ConversationInputError::InvalidIdentity);
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

id_type!(AgentRunId);
id_type!(AgentMessageId);
id_type!(AgentBatchId);
id_type!(AgentUndoGroupId);

/// Input supplied to the conversation state was outside its bounded contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationInputError {
    InvalidIdentity,
    InvalidText,
    DuplicateModel,
    DuplicateAttachment,
    DuplicateReference,
    DuplicateRun,
    ProvenanceMismatch,
    NotFound,
}

/// A configured model that can be selected for a run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentModel {
    pub provider_id: String,
    pub provider_label: String,
    pub model_id: String,
    pub model_label: String,
    pub detail: String,
}

impl AgentModel {
    pub fn new(
        provider_id: impl Into<String>,
        provider_label: impl Into<String>,
        model_id: impl Into<String>,
        model_label: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<Self, ConversationInputError> {
        let model = Self {
            provider_id: provider_id.into(),
            provider_label: provider_label.into(),
            model_id: model_id.into(),
            model_label: model_label.into(),
            detail: detail.into(),
        };
        if [
            &model.provider_id,
            &model.provider_label,
            &model.model_id,
            &model.model_label,
        ]
        .iter()
        .any(|value| !valid_text(value, MAX_ID_LENGTH))
        {
            return Err(ConversationInputError::InvalidIdentity);
        }
        if model.detail.len() > MAX_TEXT_LENGTH || model.detail.chars().any(char::is_control) {
            return Err(ConversationInputError::InvalidText);
        }
        Ok(model)
    }
}

/// Reasoning budget recorded along with model provenance.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    #[default]
    High,
    Max,
}

/// The immutable model choice captured when a run starts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSelection {
    pub model: AgentModel,
    pub effort: ReasoningEffort,
}

/// Searchable configured model palette.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalog {
    pub models: Vec<AgentModel>,
}

impl ModelCatalog {
    pub fn add(&mut self, model: AgentModel) -> Result<(), ConversationInputError> {
        if self.models.iter().any(|candidate| {
            candidate.provider_id == model.provider_id && candidate.model_id == model.model_id
        }) {
            return Err(ConversationInputError::DuplicateModel);
        }
        self.models.push(model);
        Ok(())
    }

    #[must_use]
    pub fn search(&self, query: &str, provider_id: Option<&str>) -> Vec<&AgentModel> {
        let query = query.trim().to_lowercase();
        self.models
            .iter()
            .filter(|model| provider_id.is_none_or(|provider| provider == model.provider_id))
            .filter(|model| {
                query.is_empty()
                    || [&model.provider_label, &model.model_label, &model.detail]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query))
            })
            .collect()
    }

    #[must_use]
    pub fn contains(&self, selection: &ModelSelection) -> bool {
        self.models.iter().any(|model| model == &selection.model)
    }
}

/// Semantic kind of an inline Agent Reference.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentReferenceKind {
    Screen,
    Node,
    Composition,
    Asset,
    Content,
    Property,
    Interaction,
    Diagnostic,
    Revision,
    ImportedSource,
}

impl AgentReferenceKind {
    #[must_use]
    pub const fn icon(self) -> char {
        match self {
            Self::Screen => '▣',
            Self::Node => '●',
            Self::Composition => '◇',
            Self::Asset => '▧',
            Self::Content => '▤',
            Self::Property => '◈',
            Self::Interaction => '↯',
            Self::Diagnostic => '!',
            Self::Revision => '↶',
            Self::ImportedSource => '⇩',
        }
    }
}

/// Resolution retained by a reference so old messages never silently retarget.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceResolution {
    Active,
    Renamed { current_label: String },
    Stale { explanation: String },
    Deleted { explanation: String },
    Denied { explanation: String },
}

/// A stable, typed reference embedded in an Agent message or composer context.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentReference {
    pub id: String,
    pub kind: AgentReferenceKind,
    pub target_id: String,
    pub original_label: String,
    pub resolution: ReferenceResolution,
}

impl AgentReference {
    pub fn new(
        id: impl Into<String>,
        kind: AgentReferenceKind,
        target_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Self, ConversationInputError> {
        let reference = Self {
            id: id.into(),
            kind,
            target_id: target_id.into(),
            original_label: label.into(),
            resolution: ReferenceResolution::Active,
        };
        if [&reference.id, &reference.target_id]
            .iter()
            .any(|value| !valid_text(value, MAX_ID_LENGTH))
            || reference.original_label.is_empty()
            || reference.original_label.len() > MAX_TEXT_LENGTH
            || reference.original_label.chars().any(char::is_control)
        {
            return Err(ConversationInputError::InvalidText);
        }
        Ok(reference)
    }

    pub fn mark_renamed(
        &mut self,
        current_label: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        let current_label = current_label.into();
        if !valid_text(&current_label, MAX_TEXT_LENGTH) {
            return Err(ConversationInputError::InvalidText);
        }
        self.resolution = ReferenceResolution::Renamed { current_label };
        Ok(())
    }

    pub fn mark_stale(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.resolution = ReferenceResolution::Stale {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    pub fn mark_deleted(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.resolution = ReferenceResolution::Deleted {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    pub fn mark_denied(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.resolution = ReferenceResolution::Denied {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    #[must_use]
    pub fn current_label(&self) -> &str {
        match &self.resolution {
            ReferenceResolution::Renamed { current_label } => current_label,
            _ => &self.original_label,
        }
    }

    #[must_use]
    pub fn explanation(&self) -> Option<&str> {
        match &self.resolution {
            ReferenceResolution::Stale { explanation }
            | ReferenceResolution::Deleted { explanation }
            | ReferenceResolution::Denied { explanation } => Some(explanation),
            _ => None,
        }
    }

    #[must_use]
    pub fn chip_label(&self) -> String {
        format!("{} {}", self.kind.icon(), self.current_label())
    }

    #[must_use]
    pub fn is_activatable(&self) -> bool {
        matches!(
            self.resolution,
            ReferenceResolution::Active | ReferenceResolution::Renamed { .. }
        )
    }
}

fn bounded_explanation(explanation: String) -> Result<String, ConversationInputError> {
    if !valid_text(&explanation, MAX_TEXT_LENGTH) {
        Err(ConversationInputError::InvalidText)
    } else {
        Ok(explanation)
    }
}

/// Where a scoped composer attachment came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    Upload,
    StudioLibrary,
    Url,
    GoogleDrive,
    Figma,
    Dropbox,
    OneDrive,
    CanvasSelection,
    ImportedSource,
}

/// Availability of an attachment, retaining why it cannot currently be read.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextAvailability {
    Available,
    Stale { explanation: String },
    Deleted { explanation: String },
    Denied { explanation: String },
}

/// One bounded piece of scoped context attached to a composer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextAttachment {
    pub id: String,
    pub source: ContextSource,
    pub label: String,
    pub media_type: String,
    pub availability: ContextAvailability,
    pub reference: Option<AgentReference>,
}

impl ContextAttachment {
    pub fn new(
        id: impl Into<String>,
        source: ContextSource,
        label: impl Into<String>,
        media_type: impl Into<String>,
    ) -> Result<Self, ConversationInputError> {
        let attachment = Self {
            id: id.into(),
            source,
            label: label.into(),
            media_type: media_type.into(),
            availability: ContextAvailability::Available,
            reference: None,
        };
        if !valid_text(&attachment.id, MAX_ID_LENGTH)
            || !valid_text(&attachment.label, MAX_TEXT_LENGTH)
            || !valid_text(&attachment.media_type, MAX_ID_LENGTH)
        {
            return Err(ConversationInputError::InvalidText);
        }
        Ok(attachment)
    }

    pub fn with_reference(mut self, reference: AgentReference) -> Self {
        self.reference = Some(reference);
        self
    }

    pub fn mark_stale(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.availability = ContextAvailability::Stale {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    pub fn mark_deleted(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.availability = ContextAvailability::Deleted {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    pub fn mark_denied(
        &mut self,
        explanation: impl Into<String>,
    ) -> Result<(), ConversationInputError> {
        self.availability = ContextAvailability::Denied {
            explanation: bounded_explanation(explanation.into())?,
        };
        Ok(())
    }

    #[must_use]
    pub fn explanation(&self) -> Option<&str> {
        match &self.availability {
            ContextAvailability::Stale { explanation }
            | ContextAvailability::Deleted { explanation }
            | ContextAvailability::Denied { explanation } => Some(explanation),
            ContextAvailability::Available => None,
        }
    }
}

/// Transient controls owned by the composer; no unrestricted terminal controls exist here.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComposerState {
    pub draft: String,
    pub attachments: Vec<ContextAttachment>,
    pub references: Vec<AgentReference>,
    pub import_open: bool,
    pub model_menu_open: bool,
    pub attachment_menu_open: bool,
    pub canvas_context_picking: bool,
    pub voice_active: bool,
}

impl ComposerState {
    pub fn set_draft(&mut self, draft: impl Into<String>) -> Result<(), ConversationInputError> {
        let draft = draft.into();
        if !valid_content(&draft, MAX_TEXT_LENGTH) {
            return Err(ConversationInputError::InvalidText);
        }
        self.draft = draft;
        Ok(())
    }

    pub fn attach(&mut self, attachment: ContextAttachment) -> Result<(), ConversationInputError> {
        if self.attachments.iter().any(|item| item.id == attachment.id) {
            return Err(ConversationInputError::DuplicateAttachment);
        }
        self.attachments.push(attachment);
        Ok(())
    }

    pub fn detach(&mut self, id: &str) -> bool {
        let before = self.attachments.len();
        self.attachments.retain(|item| item.id != id);
        before != self.attachments.len()
    }

    pub fn add_reference(
        &mut self,
        reference: AgentReference,
    ) -> Result<(), ConversationInputError> {
        if self.references.iter().any(|item| item.id == reference.id) {
            return Err(ConversationInputError::DuplicateReference);
        }
        self.references.push(reference);
        Ok(())
    }

    pub fn remove_reference(&mut self, id: &str) -> bool {
        let before = self.references.len();
        self.references.retain(|item| item.id != id);
        before != self.references.len()
    }

    pub fn clear_transient_menus(&mut self) -> bool {
        let was_open = self.import_open
            || self.model_menu_open
            || self.attachment_menu_open
            || self.canvas_context_picking
            || self.voice_active;
        self.import_open = false;
        self.model_menu_open = false;
        self.attachment_menu_open = false;
        self.canvas_context_picking = false;
        self.voice_active = false;
        was_open
    }

    #[must_use]
    pub fn context_count(&self) -> usize {
        self.attachments.len() + self.references.len()
    }
}

/// Whether the conversation still shows its clean welcome composer or a thread.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConversationSurface {
    #[default]
    Welcome,
    Thread,
}

/// The editor presentation that owns a conversation window.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EditorView {
    #[default]
    Focus,
    Workbench,
}

/// Conversation placement supported by the application-shell prototype.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ConversationDock {
    #[default]
    Float,
    Left,
    Right,
}

/// Logical pixel bounds retained per editor view while floating.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindowBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for WindowBounds {
    fn default() -> Self {
        Self {
            x: 40.0,
            y: 40.0,
            width: 420.0,
            height: 620.0,
        }
    }
}

impl WindowBounds {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Result<Self, ConversationInputError> {
        let bounds = Self {
            x,
            y,
            width,
            height,
        };
        if ![x, y, width, height].iter().all(|value| value.is_finite())
            || !(240.0..=4_096.0).contains(&width)
            || !(240.0..=4_096.0).contains(&height)
        {
            return Err(ConversationInputError::InvalidText);
        }
        Ok(bounds)
    }

    pub fn resize(&mut self, width: f32, height: f32) -> Result<(), ConversationInputError> {
        *self = Self::new(self.x, self.y, width, height)?;
        Ok(())
    }

    pub fn move_to(&mut self, x: f32, y: f32) -> Result<(), ConversationInputError> {
        *self = Self::new(x, y, self.width, self.height)?;
        Ok(())
    }
}

/// Floating/collapsed state, with independent geometry retained for Focus and Workbench.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloatingConversation {
    pub dock: ConversationDock,
    pub collapsed: bool,
    pub active_view: EditorView,
    pub bounds_by_view: BTreeMap<EditorView, WindowBounds>,
}

impl Default for FloatingConversation {
    fn default() -> Self {
        Self {
            dock: ConversationDock::Float,
            collapsed: false,
            active_view: EditorView::Focus,
            bounds_by_view: BTreeMap::from([(EditorView::Focus, WindowBounds::default())]),
        }
    }
}

impl FloatingConversation {
    pub fn set_dock(&mut self, dock: ConversationDock) {
        self.dock = dock;
        if dock != ConversationDock::Float {
            self.collapsed = false;
        }
    }

    pub fn switch_view(&mut self, view: EditorView) {
        self.active_view = view;
        self.bounds_by_view.entry(view).or_default();
    }

    #[must_use]
    pub fn bounds(&self) -> WindowBounds {
        self.bounds_by_view
            .get(&self.active_view)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_bounds(&mut self, bounds: WindowBounds) {
        self.bounds_by_view.insert(self.active_view, bounds);
    }

    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

/// Message role. Messages and all derived records retain run model provenance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Model/run provenance attached to messages, batches, and undo groups.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunProvenance {
    pub run_id: AgentRunId,
    pub selection: ModelSelection,
}

/// One message in a conversation thread.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentMessage {
    pub id: AgentMessageId,
    pub role: MessageRole,
    pub content: String,
    pub references: Vec<AgentReference>,
    pub provenance: RunProvenance,
}

/// One streamed command batch visible in the conversation activity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommandBatch {
    pub id: AgentBatchId,
    pub operation_id: String,
    pub base_revision: u64,
    pub committed_revision: Option<u64>,
    pub accepted: bool,
    pub provenance: RunProvenance,
}

impl AgentCommandBatch {
    pub fn new(
        id: AgentBatchId,
        operation_id: impl Into<String>,
        base_revision: u64,
        committed_revision: Option<u64>,
        accepted: bool,
        provenance: RunProvenance,
    ) -> Result<Self, ConversationInputError> {
        let operation_id = operation_id.into();
        if !valid_text(&operation_id, MAX_ID_LENGTH) {
            return Err(ConversationInputError::InvalidIdentity);
        }
        Ok(Self {
            id,
            operation_id,
            base_revision,
            committed_revision,
            accepted,
            provenance,
        })
    }
}

/// One named group of batches that undoes as a single action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentUndoGroup {
    pub id: AgentUndoGroupId,
    pub name: String,
    pub batch_ids: Vec<AgentBatchId>,
    pub provenance: RunProvenance,
}

impl AgentUndoGroup {
    pub fn new(
        id: AgentUndoGroupId,
        name: impl Into<String>,
        batch_ids: Vec<AgentBatchId>,
        provenance: RunProvenance,
    ) -> Result<Self, ConversationInputError> {
        let name = name.into();
        if !valid_text(&name, MAX_TEXT_LENGTH) {
            return Err(ConversationInputError::InvalidText);
        }
        Ok(Self {
            id,
            name,
            batch_ids,
            provenance,
        })
    }
}

/// Lifecycle of an agent run.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// A run freezes model selection even when the composer changes model later.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRun {
    pub id: AgentRunId,
    pub selection: ModelSelection,
    pub status: RunStatus,
    pub messages: Vec<AgentMessage>,
    pub batches: Vec<AgentCommandBatch>,
    pub undo_groups: Vec<AgentUndoGroup>,
}

impl AgentRun {
    pub fn new(id: AgentRunId, selection: ModelSelection) -> Self {
        Self {
            id,
            selection,
            status: RunStatus::Running,
            messages: Vec::new(),
            batches: Vec::new(),
            undo_groups: Vec::new(),
        }
    }

    #[must_use]
    pub fn provenance(&self) -> RunProvenance {
        RunProvenance {
            run_id: self.id.clone(),
            selection: self.selection.clone(),
        }
    }

    pub fn push_message(
        &mut self,
        message_id: AgentMessageId,
        role: MessageRole,
        content: String,
        references: Vec<AgentReference>,
    ) -> Result<(), ConversationInputError> {
        if content.is_empty() || !valid_content(&content, MAX_TEXT_LENGTH) {
            return Err(ConversationInputError::InvalidText);
        }
        self.messages.push(AgentMessage {
            id: message_id,
            role,
            content,
            references,
            provenance: self.provenance(),
        });
        Ok(())
    }

    pub fn push_batch(&mut self, batch: AgentCommandBatch) -> Result<(), ConversationInputError> {
        if batch.provenance.run_id != self.id {
            return Err(ConversationInputError::ProvenanceMismatch);
        }
        self.batches.push(batch);
        Ok(())
    }

    pub fn push_undo_group(&mut self, group: AgentUndoGroup) -> Result<(), ConversationInputError> {
        if group.provenance.run_id != self.id {
            return Err(ConversationInputError::ProvenanceMismatch);
        }
        self.undo_groups.push(group);
        Ok(())
    }
}

/// Result of activating an inline reference. Activation is navigation/selection only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceActivation {
    Navigate {
        kind: AgentReferenceKind,
        target_id: String,
    },
    Unavailable {
        kind: AgentReferenceKind,
        target_id: String,
        explanation: String,
    },
}

/// Keyboard input understood by the conversation surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationKey {
    Escape,
    Enter {
        shift: bool,
        ctrl: bool,
        meta: bool,
    },
    ArrowLeft {
        shift: bool,
        ctrl: bool,
        meta: bool,
    },
    ArrowRight {
        shift: bool,
        ctrl: bool,
        meta: bool,
    },
    Character {
        key: char,
        shift: bool,
        ctrl: bool,
        meta: bool,
    },
}

/// Semantic action returned to a GPUI projection after a key event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationAction {
    None,
    Send,
    InsertNewline,
    CancelTransient,
    Dock(ConversationDock),
    ToggleCollapsed,
}

/// Welcome-to-thread conversation state and its persistent floating presentation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConversationState {
    pub catalog: ModelCatalog,
    pub selected_model: Option<ModelSelection>,
    pub composer: ComposerState,
    pub floating: FloatingConversation,
    pub runs: Vec<AgentRun>,
    pub active_run_id: Option<AgentRunId>,
    pub thread_started: bool,
}

impl Default for AgentConversationState {
    fn default() -> Self {
        Self {
            catalog: ModelCatalog::default(),
            selected_model: None,
            composer: ComposerState::default(),
            floating: FloatingConversation::default(),
            runs: Vec::new(),
            active_run_id: None,
            thread_started: false,
        }
    }
}

impl AgentConversationState {
    #[must_use]
    pub fn surface(&self) -> ConversationSurface {
        if self.thread_started {
            ConversationSurface::Thread
        } else {
            ConversationSurface::Welcome
        }
    }

    pub fn select_model(
        &mut self,
        selection: ModelSelection,
    ) -> Result<(), ConversationInputError> {
        if !self.catalog.contains(&selection) {
            return Err(ConversationInputError::NotFound);
        }
        self.selected_model = Some(selection);
        self.composer.model_menu_open = false;
        Ok(())
    }

    pub fn start_run(&mut self, run_id: AgentRunId) -> Result<(), ConversationInputError> {
        let selection = self
            .selected_model
            .clone()
            .ok_or(ConversationInputError::NotFound)?;
        if self.runs.iter().any(|run| run.id == run_id) {
            return Err(ConversationInputError::DuplicateRun);
        }
        self.runs.push(AgentRun::new(run_id.clone(), selection));
        self.active_run_id = Some(run_id);
        Ok(())
    }

    /// Submit the first message or append to an existing thread. The welcome surface is
    /// replaced only after a valid message is accepted.
    pub fn submit_message(
        &mut self,
        message_id: AgentMessageId,
        run_id: AgentRunId,
    ) -> Result<(), ConversationInputError> {
        let draft = self.composer.draft.trim().to_owned();
        if draft.is_empty() || !valid_content(&draft, MAX_TEXT_LENGTH) {
            return Err(ConversationInputError::InvalidText);
        }
        if self.active_run_id.as_ref() != Some(&run_id) {
            self.start_run(run_id.clone())?;
        }
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.id == run_id)
            .ok_or(ConversationInputError::NotFound)?;
        let references = self.composer.references.clone();
        run.push_message(message_id, MessageRole::User, draft, references)?;
        self.composer.draft.clear();
        self.thread_started = true;
        Ok(())
    }

    pub fn push_assistant_message(
        &mut self,
        message_id: AgentMessageId,
        content: impl Into<String>,
        references: Vec<AgentReference>,
    ) -> Result<(), ConversationInputError> {
        let run_id = self
            .active_run_id
            .clone()
            .ok_or(ConversationInputError::NotFound)?;
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.id == run_id)
            .ok_or(ConversationInputError::NotFound)?;
        run.push_message(
            message_id,
            MessageRole::Assistant,
            content.into(),
            references,
        )
    }

    #[must_use]
    pub fn activate_reference(&self, reference: &AgentReference) -> ReferenceActivation {
        if reference.is_activatable() {
            ReferenceActivation::Navigate {
                kind: reference.kind,
                target_id: reference.target_id.clone(),
            }
        } else {
            ReferenceActivation::Unavailable {
                kind: reference.kind,
                target_id: reference.target_id.clone(),
                explanation: reference
                    .explanation()
                    .unwrap_or("Reference is unavailable.")
                    .to_owned(),
            }
        }
    }

    pub fn switch_view(&mut self, view: EditorView) {
        self.floating.switch_view(view);
    }

    pub fn finish_active_run(&mut self, status: RunStatus) -> Result<(), ConversationInputError> {
        let run_id = self
            .active_run_id
            .clone()
            .ok_or(ConversationInputError::NotFound)?;
        let run = self
            .runs
            .iter_mut()
            .find(|run| run.id == run_id)
            .ok_or(ConversationInputError::NotFound)?;
        run.status = status;
        Ok(())
    }

    #[must_use]
    pub fn handle_key(&mut self, key: ConversationKey) -> ConversationAction {
        match key {
            ConversationKey::Escape if self.composer.clear_transient_menus() => {
                ConversationAction::CancelTransient
            }
            ConversationKey::Escape => ConversationAction::None,
            ConversationKey::Enter { shift: true, .. } => ConversationAction::InsertNewline,
            ConversationKey::Enter { .. } => ConversationAction::Send,
            ConversationKey::ArrowLeft {
                shift: true,
                ctrl,
                meta,
            } if ctrl || meta => {
                self.floating.set_dock(ConversationDock::Left);
                ConversationAction::Dock(ConversationDock::Left)
            }
            ConversationKey::ArrowRight {
                shift: true,
                ctrl,
                meta,
            } if ctrl || meta => {
                self.floating.set_dock(ConversationDock::Right);
                ConversationAction::Dock(ConversationDock::Right)
            }
            ConversationKey::Character {
                key: 'f' | 'F',
                shift: true,
                ctrl,
                meta,
            } if ctrl || meta => {
                self.floating.set_dock(ConversationDock::Float);
                ConversationAction::Dock(ConversationDock::Float)
            }
            ConversationKey::Character {
                key: 'c' | 'C',
                shift: true,
                ctrl,
                meta,
            } if ctrl || meta => {
                self.floating.toggle_collapsed();
                ConversationAction::ToggleCollapsed
            }
            _ => ConversationAction::None,
        }
    }
}

/// Short alias used by GPUI view adapters.
pub type AgentConversation = AgentConversationState;

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> AgentModel {
        AgentModel::new(
            "openai",
            "OpenAI",
            "gpt-5.6",
            "GPT-5.6",
            "Fast, capable model",
        )
        .unwrap()
    }

    fn selection() -> ModelSelection {
        ModelSelection {
            model: model(),
            effort: ReasoningEffort::High,
        }
    }

    #[test]
    fn first_message_transitions_welcome_and_freezes_model_provenance() {
        let mut conversation = AgentConversationState::default();
        conversation.catalog.add(model()).unwrap();
        conversation.select_model(selection()).unwrap();
        conversation
            .composer
            .set_draft("Make the badge clearer")
            .unwrap();
        conversation
            .start_run(AgentRunId::new("run-1").unwrap())
            .unwrap();
        conversation
            .submit_message(
                AgentMessageId::new("message-1").unwrap(),
                AgentRunId::new("run-1").unwrap(),
            )
            .unwrap();

        assert!(conversation.thread_started);
        assert!(conversation.composer.draft.is_empty());
        assert_eq!(
            conversation.runs[0].messages[0].provenance.selection,
            selection()
        );
    }

    #[test]
    fn model_search_and_run_selection_are_scoped_to_configured_models() {
        let mut catalog = ModelCatalog::default();
        catalog.add(model()).unwrap();
        catalog
            .add(
                AgentModel::new("anthropic", "Anthropic", "claude", "Claude", "Reasoning").unwrap(),
            )
            .unwrap();
        assert_eq!(catalog.search("gpt", None).len(), 1);
        assert_eq!(catalog.search("", Some("anthropic")).len(), 1);
    }

    #[test]
    fn references_keep_identity_when_renamed_and_explain_unavailable_targets() {
        let mut renamed =
            AgentReference::new("ref-1", AgentReferenceKind::Node, "node-1", "Sale badge").unwrap();
        renamed.mark_renamed("Promotion badge").unwrap();
        assert_eq!(renamed.current_label(), "Promotion badge");
        assert!(renamed.is_activatable());

        let mut deleted = renamed.clone();
        deleted
            .mark_deleted("The node was deleted in revision 12.")
            .unwrap();
        assert!(!deleted.is_activatable());
        assert_eq!(
            deleted.explanation(),
            Some("The node was deleted in revision 12.")
        );
    }

    #[test]
    fn floating_bounds_and_docking_survive_view_switches() {
        let mut conversation = AgentConversationState::default();
        conversation
            .floating
            .set_bounds(WindowBounds::new(20.0, 30.0, 500.0, 700.0).unwrap());
        conversation.switch_view(EditorView::Workbench);
        conversation.floating.set_dock(ConversationDock::Right);
        conversation
            .floating
            .set_bounds(WindowBounds::new(0.0, 0.0, 360.0, 900.0).unwrap());
        conversation.switch_view(EditorView::Focus);
        assert_eq!(conversation.floating.dock, ConversationDock::Right);
        assert_eq!(conversation.floating.bounds().width, 500.0);
        conversation.switch_view(EditorView::Workbench);
        assert_eq!(conversation.floating.bounds().width, 360.0);
    }

    #[test]
    fn keyboard_escape_cancels_transient_state_and_shortcuts_dock() {
        let mut conversation = AgentConversationState::default();
        conversation.composer.model_menu_open = true;
        assert_eq!(
            conversation.handle_key(ConversationKey::Escape),
            ConversationAction::CancelTransient
        );
        assert_eq!(
            conversation.handle_key(ConversationKey::ArrowLeft {
                shift: true,
                ctrl: true,
                meta: false,
            }),
            ConversationAction::Dock(ConversationDock::Left)
        );
        assert_eq!(conversation.floating.dock, ConversationDock::Left);
    }

    #[test]
    fn composer_keeps_scoped_context_and_explains_unavailable_attachments() {
        let mut attachment = ContextAttachment::new(
            "upload-1",
            ContextSource::Upload,
            "brief.md",
            "text/markdown",
        )
        .unwrap();
        attachment
            .mark_deleted("The imported file was removed from the project.")
            .unwrap();
        assert_eq!(
            attachment.explanation(),
            Some("The imported file was removed from the project.")
        );

        let mut composer = ComposerState::default();
        composer.attach(attachment).unwrap();
        composer
            .add_reference(
                AgentReference::new(
                    "ref-1",
                    AgentReferenceKind::ImportedSource,
                    "source-1",
                    "brief.md",
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(composer.context_count(), 2);
        assert!(composer.detach("upload-1"));
        assert!(composer.remove_reference("ref-1"));
        assert_eq!(composer.context_count(), 0);
    }

    #[test]
    fn multiline_composer_content_is_bounded_but_control_bytes_are_rejected() {
        let mut composer = ComposerState::default();
        composer.set_draft("first line\nsecond line").unwrap();
        assert!(composer.set_draft("bad\0input").is_err());
    }
}
