//! Typed settings and support surfaces for Studio Designer.
//!
//! The records in this module are deliberately independent of GPUI, identity
//! implementations, and persistence engines.  A native shell can apply the
//! returned [`SettingsEffect`] immediately and persist the same typed record
//! through [`SettingsPersistence`].

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SafeDiagnostic;
use studio_security::SensitiveValueFilter;

/// The current on-disk settings schema.
pub const SETTINGS_SCHEMA_VERSION: u16 = 1;

/// Maximum size of a project identity that may cross a settings/recovery route boundary.
const MAX_PROJECT_ID_BYTES: usize = 128;

/// Theme selected by the designer.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    /// Follow the operating-system theme.
    #[default]
    System,
    /// Use the light theme.
    Light,
    /// Use the dark theme.
    Dark,
    /// Use the high-contrast theme.
    HighContrast,
}

/// Display language for first-party Studio surfaces.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    /// English copy.
    #[default]
    English,
    /// Spanish copy.
    Spanish,
    /// French copy.
    French,
    /// German copy.
    German,
    /// Japanese copy.
    Japanese,
}

/// Accessibility options that affect rendering and assistive technology.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilitySettings {
    /// Increase text and control sizing.
    pub large_text: bool,
    /// Prefer stronger borders and contrast.
    pub high_contrast: bool,
    /// Show additional descriptions for controls.
    pub descriptive_labels: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            large_text: false,
            high_contrast: false,
            descriptive_labels: true,
        }
    }
}

/// Keyboard behavior preferences.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardSettings {
    /// Enable keyboard shortcuts and focus traversal.
    pub shortcuts_enabled: bool,
    /// Use vim-style navigation where a surface supports it.
    pub vim_navigation: bool,
    /// Show shortcut hints beside commands.
    pub show_shortcuts: bool,
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            shortcuts_enabled: true,
            vim_navigation: false,
            show_shortcuts: true,
        }
    }
}

/// Startup behavior for the Designer shell.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    /// Open the identity chooser.
    #[default]
    IdentityChooser,
    /// Open the dashboard after identity discovery.
    Dashboard,
    /// Resume the last project when it is available.
    ResumeLastProject,
}

/// Startup preferences.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartupSettings {
    /// Initial destination after startup identity discovery.
    pub mode: StartupMode,
    /// Whether the product welcome is shown again on startup.
    pub show_welcome: bool,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self {
            mode: StartupMode::IdentityChooser,
            show_welcome: true,
        }
    }
}

/// Autosave policy for Designer revisions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutosaveSettings {
    /// Enable autosave.
    pub enabled: bool,
    /// Delay between the last edit and an autosave, in milliseconds.
    pub delay_ms: u32,
    /// Write a recovery snapshot before autosave replaces durable state.
    pub recovery_snapshot: bool,
}

impl Default for AutosaveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 750,
            recovery_snapshot: true,
        }
    }
}

/// Notification preferences.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationSettings {
    /// Show success notifications.
    pub success: bool,
    /// Show warning and failure notifications.
    pub warnings: bool,
    /// Play notification sounds.
    pub sounds: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            success: true,
            warnings: true,
            sounds: false,
        }
    }
}

/// Consent state for including redacted diagnostics in feedback.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticConsent {
    /// The user has not made a choice.
    #[default]
    NotSet,
    /// The user explicitly declined diagnostic sharing.
    Denied,
    /// The user explicitly allowed diagnostic sharing.
    Granted,
}

/// Release channel used for update checks.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    /// Stable releases only.
    #[default]
    Stable,
    /// Preview releases for evaluation.
    Preview,
    /// Development builds and nightly updates.
    Nightly,
}

/// Complete global application settings record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSettings {
    /// Settings schema version.
    pub schema_version: u16,
    /// Theme preference.
    pub theme: ThemePreference,
    /// Display language.
    pub language: LanguagePreference,
    /// Accessibility options other than reduced motion.
    pub accessibility: AccessibilitySettings,
    /// Disable movement and animated transitions.
    pub reduced_motion: bool,
    /// Keyboard behavior.
    pub keyboard: KeyboardSettings,
    /// Startup behavior.
    pub startup: StartupSettings,
    /// Autosave defaults for new projects.
    pub autosave: AutosaveSettings,
    /// Notification behavior.
    pub notifications: NotificationSettings,
    /// Diagnostic-sharing consent.
    pub diagnostic_sharing: DiagnosticConsent,
    /// Update release channel.
    pub update_channel: UpdateChannel,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme: ThemePreference::System,
            language: LanguagePreference::English,
            accessibility: AccessibilitySettings::default(),
            reduced_motion: false,
            keyboard: KeyboardSettings::default(),
            startup: StartupSettings::default(),
            autosave: AutosaveSettings::default(),
            notifications: NotificationSettings::default(),
            diagnostic_sharing: DiagnosticConsent::NotSet,
            update_channel: UpdateChannel::Stable,
        }
    }
}

impl GlobalSettings {
    /// Create settings with the current schema version and defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// A typed global setting update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GlobalSettingChange {
    /// Change the theme.
    Theme(ThemePreference),
    /// Change the language.
    Language(LanguagePreference),
    /// Replace accessibility options.
    Accessibility(AccessibilitySettings),
    /// Change reduced-motion behavior.
    ReducedMotion(bool),
    /// Replace keyboard options.
    Keyboard(KeyboardSettings),
    /// Replace startup options.
    Startup(StartupSettings),
    /// Replace autosave defaults.
    Autosave(AutosaveSettings),
    /// Replace notification options.
    Notifications(NotificationSettings),
    /// Change diagnostic-sharing consent.
    DiagnosticSharing(DiagnosticConsent),
    /// Change the update channel.
    UpdateChannel(UpdateChannel),
}

/// A host-visible effect produced by a persisted setting update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsEffect {
    /// Apply a new theme immediately.
    ThemeChanged(ThemePreference),
    /// Reload translated copy immediately.
    LanguageChanged(LanguagePreference),
    /// Apply accessibility options immediately.
    AccessibilityChanged(AccessibilitySettings),
    /// Apply reduced-motion behavior immediately.
    ReducedMotionChanged(bool),
    /// Apply keyboard behavior immediately.
    KeyboardChanged(KeyboardSettings),
    /// Apply startup behavior for the next launch.
    StartupChanged(StartupSettings),
    /// Apply autosave behavior immediately.
    AutosaveChanged(AutosaveSettings),
    /// Apply notification behavior immediately.
    NotificationsChanged(NotificationSettings),
    /// Update consent used by feedback surfaces.
    DiagnosticSharingChanged(DiagnosticConsent),
    /// Update channel changed.
    UpdateChannelChanged(UpdateChannel),
    /// A project setting changed and its panel should refresh.
    ProjectChanged { project_id: String },
}

impl GlobalSettings {
    fn apply(&mut self, change: GlobalSettingChange) -> SettingsEffect {
        match change {
            GlobalSettingChange::Theme(value) => {
                self.theme = value;
                SettingsEffect::ThemeChanged(value)
            }
            GlobalSettingChange::Language(value) => {
                self.language = value;
                SettingsEffect::LanguageChanged(value)
            }
            GlobalSettingChange::Accessibility(value) => {
                self.accessibility = value.clone();
                SettingsEffect::AccessibilityChanged(value)
            }
            GlobalSettingChange::ReducedMotion(value) => {
                self.reduced_motion = value;
                SettingsEffect::ReducedMotionChanged(value)
            }
            GlobalSettingChange::Keyboard(value) => {
                self.keyboard = value.clone();
                SettingsEffect::KeyboardChanged(value)
            }
            GlobalSettingChange::Startup(value) => {
                self.startup = value.clone();
                SettingsEffect::StartupChanged(value)
            }
            GlobalSettingChange::Autosave(value) => {
                self.autosave = value.clone();
                SettingsEffect::AutosaveChanged(value)
            }
            GlobalSettingChange::Notifications(value) => {
                self.notifications = value.clone();
                SettingsEffect::NotificationsChanged(value)
            }
            GlobalSettingChange::DiagnosticSharing(value) => {
                self.diagnostic_sharing = value;
                SettingsEffect::DiagnosticSharingChanged(value)
            }
            GlobalSettingChange::UpdateChannel(value) => {
                self.update_channel = value;
                SettingsEffect::UpdateChannelChanged(value)
            }
        }
    }
}

/// Project metadata shown in the project settings surface.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataSettings {
    /// Human-readable project name.
    pub name: String,
    /// Optional project description.
    pub description: String,
}

/// Local/cloud storage and synchronization behavior.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    /// Store project data locally.
    #[default]
    Local,
    /// Store project data in the configured cloud workspace.
    Cloud,
}

/// Project storage and synchronization settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageSyncSettings {
    /// Authoritative storage mode.
    pub mode: StorageMode,
    /// Whether synchronization is enabled.
    pub sync_enabled: bool,
    /// Keep a local copy when cloud sync is enabled.
    pub keep_local_copy: bool,
}

/// Opaque Runtime identity selection.  Authentication and provisioning remain ticket 54 seams.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIdentitySettings {
    /// Host-owned identity reference, never a password or credential.
    pub identity_ref: Option<String>,
    /// Cached display label supplied by the identity integration.
    pub display_name: Option<String>,
}

/// Build and signing policy for a project.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSigningSettings {
    /// Named host-owned signing profile.
    pub signing_profile: Option<String>,
    /// Require a valid signature before launch.
    pub require_signature: bool,
    /// Build channel metadata.
    pub channel: UpdateChannel,
}

/// Project extension admission settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionSettings {
    /// Enabled extension IDs in deterministic order.
    pub enabled: BTreeSet<String>,
}

/// Project capability admission settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySettings {
    /// Capabilities explicitly allowed for this project.
    pub allowed: BTreeSet<String>,
}

/// Runtime data settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataSettings {
    /// Preserve data when a preview session ends.
    pub persist_preview_data: bool,
    /// Permit a user-confirmed data reset from the project surface.
    pub allow_reset: bool,
}

/// Recovery and snapshot settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySettings {
    /// Enable logical recovery snapshots.
    pub snapshots_enabled: bool,
    /// Number of snapshots retained.
    pub retained_snapshots: u16,
    /// Offer recovery after an interrupted open.
    pub offer_on_startup: bool,
}

/// Complete settings for one project.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSettings {
    /// Settings schema version.
    pub schema_version: u16,
    /// Stable project key owned by the dashboard/session seam.
    pub project_id: String,
    /// Project metadata.
    pub metadata: MetadataSettings,
    /// Storage and sync policy.
    pub storage_sync: StorageSyncSettings,
    /// Opaque Runtime identity selection.
    pub runtime_identity: RuntimeIdentitySettings,
    /// Build/signing policy.
    pub build_signing: BuildSigningSettings,
    /// Extension admission.
    pub extensions: ExtensionSettings,
    /// Capability admission.
    pub capabilities: CapabilitySettings,
    /// Runtime data policy.
    pub data: DataSettings,
    /// Recovery policy.
    pub recovery: RecoverySettings,
}

impl ProjectSettings {
    /// Create project settings with deterministic defaults.
    #[must_use]
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            project_id: project_id.into(),
            metadata: MetadataSettings::default(),
            storage_sync: StorageSyncSettings {
                mode: StorageMode::Local,
                sync_enabled: false,
                keep_local_copy: true,
            },
            runtime_identity: RuntimeIdentitySettings::default(),
            build_signing: BuildSigningSettings {
                signing_profile: None,
                require_signature: true,
                channel: UpdateChannel::Stable,
            },
            extensions: ExtensionSettings::default(),
            capabilities: CapabilitySettings::default(),
            data: DataSettings {
                persist_preview_data: false,
                allow_reset: true,
            },
            recovery: RecoverySettings {
                snapshots_enabled: true,
                retained_snapshots: 5,
                offer_on_startup: true,
            },
        }
    }

    fn apply(&mut self, change: ProjectSettingChange) {
        match change {
            ProjectSettingChange::Metadata(value) => self.metadata = value,
            ProjectSettingChange::StorageSync(value) => self.storage_sync = value,
            ProjectSettingChange::RuntimeIdentity(value) => self.runtime_identity = value,
            ProjectSettingChange::BuildSigning(value) => self.build_signing = value,
            ProjectSettingChange::Extensions(value) => self.extensions = value,
            ProjectSettingChange::Capabilities(value) => self.capabilities = value,
            ProjectSettingChange::Data(value) => self.data = value,
            ProjectSettingChange::Recovery(value) => self.recovery = value,
        }
    }
}

/// A typed project setting update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectSettingChange {
    /// Replace metadata.
    Metadata(MetadataSettings),
    /// Replace storage/sync settings.
    StorageSync(StorageSyncSettings),
    /// Replace the opaque Runtime identity reference.
    RuntimeIdentity(RuntimeIdentitySettings),
    /// Replace build/signing settings.
    BuildSigning(BuildSigningSettings),
    /// Replace extension admission.
    Extensions(ExtensionSettings),
    /// Replace capability admission.
    Capabilities(CapabilitySettings),
    /// Replace data settings.
    Data(DataSettings),
    /// Replace recovery settings.
    Recovery(RecoverySettings),
}

/// Stable persistence error categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsErrorCode {
    /// A caller supplied a value that cannot safely cross a settings boundary.
    InvalidInput,
    /// The persistence backend is unavailable.
    Unavailable,
    /// Stored settings cannot be decoded safely.
    Corrupt,
    /// Stored settings use an unsupported schema.
    Incompatible,
    /// The write was rejected by the persistence backend.
    Rejected,
}

/// Safe settings persistence error.
#[derive(Clone, Debug, Deserialize, Error, Eq, PartialEq, Serialize)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct SettingsError {
    /// Stable failure category.
    pub code: SettingsErrorCode,
    /// User-safe explanation.
    pub message: String,
}

impl SettingsError {
    fn invalid_project_id() -> Self {
        Self {
            code: SettingsErrorCode::InvalidInput,
            message: "project identity is invalid for settings".to_owned(),
        }
    }

    fn incompatible() -> Self {
        Self {
            code: SettingsErrorCode::Incompatible,
            message: "settings schema is not supported by this Studio version".to_owned(),
        }
    }
}

/// Persistence adapter for global and project settings.
pub trait SettingsPersistence: Send + Sync {
    /// Load global settings, or `None` for a new device.
    fn load_global(&self) -> Result<Option<GlobalSettings>, SettingsError>;
    /// Atomically persist global settings.
    fn save_global(&self, settings: &GlobalSettings) -> Result<(), SettingsError>;
    /// Load one project settings record.
    fn load_project(&self, project_id: &str) -> Result<Option<ProjectSettings>, SettingsError>;
    /// Atomically persist one project settings record.
    fn save_project(&self, settings: &ProjectSettings) -> Result<(), SettingsError>;
}

/// Deterministic process-local settings persistence for tests and previews.
#[derive(Clone, Default)]
pub struct InMemorySettingsPersistence {
    global: Arc<Mutex<Option<GlobalSettings>>>,
    projects: Arc<Mutex<BTreeMap<String, ProjectSettings>>>,
}

impl InMemorySettingsPersistence {
    /// Read a project directly for adapter assertions.
    #[must_use]
    pub fn project(&self, project_id: &str) -> Option<ProjectSettings> {
        self.projects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned()
    }
}

impl SettingsPersistence for InMemorySettingsPersistence {
    fn load_global(&self) -> Result<Option<GlobalSettings>, SettingsError> {
        Ok(self
            .global
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }
    fn save_global(&self, settings: &GlobalSettings) -> Result<(), SettingsError> {
        *self
            .global
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(settings.clone());
        Ok(())
    }
    fn load_project(&self, project_id: &str) -> Result<Option<ProjectSettings>, SettingsError> {
        Ok(self
            .projects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned())
    }
    fn save_project(&self, settings: &ProjectSettings) -> Result<(), SettingsError> {
        self.projects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(settings.project_id.clone(), settings.clone());
        Ok(())
    }
}

/// Controller that applies settings and persists them through a typed seam.
pub struct SettingsController<P> {
    persistence: P,
    global: GlobalSettings,
    effects: Vec<SettingsEffect>,
}

impl<P: SettingsPersistence> SettingsController<P> {
    /// Open settings from persistence, using defaults for a fresh device.
    pub fn new(persistence: P) -> Result<Self, SettingsError> {
        let global = persistence
            .load_global()?
            .unwrap_or_else(GlobalSettings::new);
        if global.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(SettingsError::incompatible());
        }
        Ok(Self {
            persistence,
            global,
            effects: Vec::new(),
        })
    }

    /// Current global settings.
    #[must_use]
    pub const fn global(&self) -> &GlobalSettings {
        &self.global
    }

    /// Persist and immediately publish one global setting effect.
    pub fn update_global(
        &mut self,
        change: GlobalSettingChange,
    ) -> Result<SettingsEffect, SettingsError> {
        let previous = self.global.clone();
        let effect = self.global.apply(change);
        if let Err(error) = self.persistence.save_global(&self.global) {
            self.global = previous;
            return Err(error);
        }
        self.effects.push(effect.clone());
        Ok(effect)
    }

    /// Load one project, applying defaults for a newly created project.
    pub fn project(&self, project_id: &str) -> Result<ProjectSettings, SettingsError> {
        if !valid_project_id(project_id) {
            return Err(SettingsError::invalid_project_id());
        }
        let settings = self
            .persistence
            .load_project(project_id)?
            .unwrap_or_else(|| ProjectSettings::new(project_id));
        if settings.schema_version != SETTINGS_SCHEMA_VERSION || settings.project_id != project_id {
            return Err(SettingsError::incompatible());
        }
        Ok(settings)
    }

    /// Persist and immediately publish one project setting effect.
    pub fn update_project(
        &mut self,
        project_id: &str,
        change: ProjectSettingChange,
    ) -> Result<SettingsEffect, SettingsError> {
        let mut settings = self.project(project_id)?;
        settings.apply(change);
        self.persistence.save_project(&settings)?;
        let effect = SettingsEffect::ProjectChanged {
            project_id: project_id.to_owned(),
        };
        self.effects.push(effect.clone());
        Ok(effect)
    }

    /// Drain effects in update order for native UI application.
    pub fn take_effects(&mut self) -> Vec<SettingsEffect> {
        std::mem::take(&mut self.effects)
    }
}

/// Explicit consent attached to a feedback send operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FeedbackConsent {
    /// No explicit send permission was given.
    #[default]
    NotGranted,
    /// The user explicitly allowed this feedback to be sent.
    Granted,
}

/// User-entered feedback request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedbackDraft {
    /// Explicit message written by the user.
    pub message: String,
    /// Whether the user asked to attach diagnostics.
    pub include_diagnostics: bool,
    /// Explicit consent to send this feedback.
    pub consent: FeedbackConsent,
}

/// Redacted diagnostic payload safe to send with feedback.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Sanitized diagnostic message.
    pub message: String,
}

/// Feedback payload passed to an external transport.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackPayload {
    /// Explicit user message.
    pub message: String,
    /// Redacted diagnostics, empty unless both consent gates allow them.
    pub diagnostics: Vec<RedactedDiagnostic>,
}

/// Feedback failure with no transport or backend details.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FeedbackError {
    /// The user must explicitly consent before a send is attempted.
    #[error("explicit feedback consent is required")]
    ConsentRequired,
    /// A non-empty message is required.
    #[error("feedback message is required")]
    MessageRequired,
    /// Feedback message exceeds the stable UI limit.
    #[error("feedback message is too long")]
    MessageTooLong,
    /// The transport rejected the payload.
    #[error("feedback could not be sent")]
    Transport,
}

/// Host-owned feedback transport seam.
pub trait FeedbackTransport {
    /// Send one already-redacted payload.
    fn send(&mut self, payload: &FeedbackPayload) -> Result<(), FeedbackError>;
}

/// Feedback service enforcing both send and diagnostic consent.
pub struct FeedbackService<T> {
    transport: T,
    filter: SensitiveValueFilter,
    diagnostic_consent: DiagnosticConsent,
}

impl<T: FeedbackTransport> FeedbackService<T> {
    /// Create a service with the global diagnostic-sharing choice.
    #[must_use]
    pub fn new(
        transport: T,
        diagnostic_consent: DiagnosticConsent,
        filter: SensitiveValueFilter,
    ) -> Self {
        Self {
            transport,
            filter,
            diagnostic_consent,
        }
    }

    /// Submit feedback.  No transport call occurs without explicit consent.
    pub fn submit(
        &mut self,
        draft: &FeedbackDraft,
        diagnostics: &[SafeDiagnostic],
    ) -> Result<(), FeedbackError> {
        if draft.consent != FeedbackConsent::Granted {
            return Err(FeedbackError::ConsentRequired);
        }
        let message = draft.message.trim();
        if message.is_empty() {
            return Err(FeedbackError::MessageRequired);
        }
        if message.chars().count() > 16_384 {
            return Err(FeedbackError::MessageTooLong);
        }
        let diagnostics =
            if draft.include_diagnostics && self.diagnostic_consent == DiagnosticConsent::Granted {
                diagnostics
                    .iter()
                    .map(|diagnostic| RedactedDiagnostic {
                        code: self.filter.sanitize(diagnostic.code()),
                        message: self.filter.sanitize(diagnostic.message()),
                    })
                    .collect()
            } else {
                Vec::new()
            };
        self.transport.send(&FeedbackPayload {
            message: message.to_owned(),
            diagnostics,
        })
    }

    /// Replace the global diagnostic-sharing choice for future sends.
    pub fn set_diagnostic_consent(&mut self, consent: DiagnosticConsent) {
        self.diagnostic_consent = consent;
    }
}

fn valid_project_id(project_id: &str) -> bool {
    !project_id.is_empty()
        && project_id.len() <= MAX_PROJECT_ID_BYTES
        && !project_id.chars().any(char::is_control)
        && !project_id.contains(['/', '\\', '?', '#'])
}

/// Core workflows covered by Help.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HelpTopic {
    /// Product welcome workflow.
    Welcome,
    /// Identity and session workflow.
    Identity,
    /// Project Dashboard workflow.
    Dashboard,
    /// Studio Design editing workflow.
    Design,
    /// Agent-assisted authoring workflow.
    Agents,
    /// Prototype and preview workflow.
    Preview,
    /// Build and signing workflow.
    Build,
    /// Library workflow.
    Library,
    /// Extension and template workflow.
    Extensions,
    /// Recovery workflow.
    Recovery,
    /// Keyboard command and focus workflow.
    Keyboard,
}

/// One deterministic Help article.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpArticle {
    /// Stable topic ID.
    pub topic: HelpTopic,
    /// Article title.
    pub title: &'static str,
    /// Short explanation.
    pub summary: &'static str,
    /// Ordered workflow steps.
    pub steps: &'static [&'static str],
}

impl HelpTopic {
    /// Return the first-party article for this topic.
    #[must_use]
    pub const fn article(self) -> HelpArticle {
        match self {
            Self::Welcome => HelpArticle {
                topic: self,
                title: "Welcome to Studio",
                summary: "Choose an identity and enter the Designer.",
                steps: &[
                    "Choose a Local or Cloud Identity.",
                    "Create or unlock the identity.",
                    "Continue to the Project Dashboard.",
                ],
            },
            Self::Identity => HelpArticle {
                topic: self,
                title: "Identity and sessions",
                summary: "Manage local sign-in without requiring a network.",
                steps: &[
                    "Select an identity.",
                    "Sign in or unlock it with its password.",
                    "Revoke remembered sessions from an authenticated identity.",
                ],
            },
            Self::Dashboard => HelpArticle {
                topic: self,
                title: "Project Dashboard",
                summary: "Find, create, import, and organize projects.",
                steps: &[
                    "Search or filter the catalog.",
                    "Switch Grid, Index, or Activity views.",
                    "Open a project or use its lifecycle menu.",
                ],
            },
            Self::Design => HelpArticle {
                topic: self,
                title: "Design a screen",
                summary: "Edit Studio Design through native controls.",
                steps: &[
                    "Select a screen and node.",
                    "Use the inspector or canvas tools.",
                    "Undo a named change when needed.",
                ],
            },
            Self::Agents => HelpArticle {
                topic: self,
                title: "Agents and conversations",
                summary: "Use the host-owned agent channel without exposing project secrets.",
                steps: &[
                    "Open a conversation from the project shell.",
                    "Review proposed changes and diagnostics before applying them.",
                    "Keep protected credentials and capabilities host-owned.",
                ],
            },
            Self::Preview => HelpArticle {
                topic: self,
                title: "Preview and test",
                summary: "Exercise declared interactions against isolated state.",
                steps: &[
                    "Choose a screen or route.",
                    "Start prototype mode.",
                    "Return to editing without changing the design.",
                ],
            },
            Self::Build => HelpArticle {
                topic: self,
                title: "Build and sign",
                summary: "Validate and package a Runtime Application.",
                steps: &[
                    "Resolve readiness diagnostics.",
                    "Choose a signing profile.",
                    "Build, sign, and launch the verified package.",
                ],
            },
            Self::Library => HelpArticle {
                topic: self,
                title: "Studio Library",
                summary: "Manage assets and typed content owned by a project.",
                steps: &[
                    "Admit an asset or collection.",
                    "Review provenance and diagnostics.",
                    "Bind it to a design property.",
                ],
            },
            Self::Extensions => HelpArticle {
                topic: self,
                title: "Extensions and templates",
                summary: "Admit signed extensions and templates through project settings.",
                steps: &[
                    "Review the extension identity and requested capabilities.",
                    "Grant only the capabilities the project needs.",
                    "Disable or remove an extension from its project settings.",
                ],
            },
            Self::Recovery => HelpArticle {
                topic: self,
                title: "Recovery",
                summary: "Restore a project from a safe logical snapshot.",
                steps: &[
                    "Open the recovery center.",
                    "Review the snapshot and journal status.",
                    "Choose restore or keep the current revision.",
                ],
            },
            Self::Keyboard => HelpArticle {
                topic: self,
                title: "Keyboard commands",
                summary: "Move through shell surfaces with predictable focus and shortcuts.",
                steps: &[
                    "Use Tab or Arrow keys to move focus.",
                    "Press Enter to activate the focused surface.",
                    "Press Escape to dismiss the current support surface.",
                ],
            },
        }
    }
}

/// Keyboard navigation model for Help articles.
#[derive(Clone, Debug)]
pub struct HelpNavigator {
    topics: Vec<HelpTopic>,
    index: usize,
}

impl Default for HelpNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpNavigator {
    /// Create the stable Help topic order.
    #[must_use]
    pub fn new() -> Self {
        Self {
            topics: vec![
                HelpTopic::Welcome,
                HelpTopic::Identity,
                HelpTopic::Dashboard,
                HelpTopic::Design,
                HelpTopic::Agents,
                HelpTopic::Preview,
                HelpTopic::Build,
                HelpTopic::Library,
                HelpTopic::Extensions,
                HelpTopic::Recovery,
                HelpTopic::Keyboard,
            ],
            index: 0,
        }
    }
    /// Move to the next article, wrapping at the end.
    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.topics.len();
    }
    /// Move to the previous article, wrapping at the beginning.
    pub fn previous(&mut self) {
        self.index = (self.index + self.topics.len() - 1) % self.topics.len();
    }
    /// Return the selected article.
    #[must_use]
    pub fn current(&self) -> HelpArticle {
        self.topics[self.index].article()
    }
    /// Return all topics in keyboard order.
    #[must_use]
    pub fn topics(&self) -> &[HelpTopic] {
        &self.topics
    }
}

/// Top-level support surfaces exposed by consistent application navigation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SupportSurface {
    /// Global and project settings.
    Settings,
    /// Help articles.
    Help,
    /// Feedback form.
    Feedback,
    /// Product information and notices.
    About,
}

/// Keyboard actions understood by support-surface navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardAction {
    /// Move focus forward.
    Tab,
    /// Move focus backward.
    ShiftTab,
    /// Move focus down.
    ArrowDown,
    /// Move focus up.
    ArrowUp,
    /// Activate the focused surface.
    Enter,
    /// Dismiss the current surface.
    Escape,
}

/// Result of handling a support keyboard action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationResult {
    /// Focus moved to a surface.
    Focused(SupportSurface),
    /// The focused surface was activated.
    Activated(SupportSurface),
    /// The current surface was dismissed.
    Dismissed,
}

/// Stable focus and activation order for Settings, Help, Feedback, and About.
#[derive(Clone, Debug, Default)]
pub struct SupportNavigator {
    index: usize,
}

impl SupportNavigator {
    /// Stable keyboard order shared by menus and command surfaces.
    pub const ORDER: [SupportSurface; 4] = [
        SupportSurface::Settings,
        SupportSurface::Help,
        SupportSurface::Feedback,
        SupportSurface::About,
    ];
    /// Create a navigator focused on Settings.
    #[must_use]
    pub const fn new() -> Self {
        Self { index: 0 }
    }
    /// Current focused surface.
    #[must_use]
    pub const fn current(&self) -> SupportSurface {
        Self::ORDER[self.index]
    }
    /// Handle a keyboard action with wrapping focus and activation.
    #[must_use]
    pub fn handle(&mut self, action: KeyboardAction) -> NavigationResult {
        match action {
            KeyboardAction::Tab | KeyboardAction::ArrowDown => {
                self.index = (self.index + 1) % Self::ORDER.len()
            }
            KeyboardAction::ShiftTab | KeyboardAction::ArrowUp => {
                self.index = (self.index + Self::ORDER.len() - 1) % Self::ORDER.len()
            }
            KeyboardAction::Enter => return NavigationResult::Activated(self.current()),
            KeyboardAction::Escape => return NavigationResult::Dismissed,
        }
        NavigationResult::Focused(self.current())
    }
}

/// One license or notice displayed by About.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseNotice {
    /// Dependency or component name.
    pub name: String,
    /// SPDX or human-readable license identifier.
    pub license: String,
    /// Where the complete notice is recorded.
    pub source: String,
}

/// About data shown by the native shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AboutInfo {
    /// Product name.
    pub product_name: String,
    /// Product version.
    pub version: String,
    /// Release channel.
    pub channel: UpdateChannel,
    /// License and notice entries.
    pub notices: Vec<LicenseNotice>,
}

impl AboutInfo {
    /// Construct About data from release metadata and notices supplied by packaging.
    #[must_use]
    pub fn new(
        product_name: impl Into<String>,
        version: impl Into<String>,
        channel: UpdateChannel,
        notices: Vec<LicenseNotice>,
    ) -> Self {
        Self {
            product_name: product_name.into(),
            version: version.into(),
            channel,
            notices,
        }
    }

    /// Construct the minimum Studio notice set from the checked-in notice document.
    #[must_use]
    pub fn studio(version: impl Into<String>, channel: UpdateChannel) -> Self {
        Self::new(
            "Studio Designer",
            version,
            channel,
            vec![
                LicenseNotice {
                    name: "Studio Runtime components".to_owned(),
                    license: "Apache-2.0".to_owned(),
                    source: "THIRD_PARTY_NOTICES.md".to_owned(),
                },
                LicenseNotice {
                    name: "Zed GPUI".to_owned(),
                    license: "GPL-3.0-or-later (release review required)".to_owned(),
                    source: "THIRD_PARTY_NOTICES.md".to_owned(),
                },
            ],
        )
    }
}
