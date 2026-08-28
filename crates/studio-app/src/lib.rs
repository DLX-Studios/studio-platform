//! Studio's Wayland-only native shell and host-owned foundation surfaces.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::assigning_clones,
    clippy::if_not_else,
    clippy::manual_let_else,
    clippy::self_only_used_in_recursion,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::semicolon_if_nothing_returned,
    clippy::needless_pass_by_value
)]

mod action_dispatch;
pub mod agent_conversation;
pub mod bootstrap;
pub mod cli;
mod confirmation_surface;
pub mod connection;
mod diagnostic;
mod failure_surface;
pub mod focus_view;
pub mod foundation;
pub mod host;
pub mod identity_shell;
pub mod plugin_surface;
mod preferences;
mod print_preview;
pub mod project_dashboard;
pub mod resilience;
mod router;
pub mod settings;
mod shutdown;

pub use action_dispatch::{NativeCheckoutError, NativeCheckoutShell};
pub use bootstrap::{
    BootstrapError, LocalStoreDashboardPersistence, LocalStoreSettingsPersistence,
    NativeProductBootstrap, NativeProductShell, NativeProductState, OfflineSyncWorker,
    ProductRoute,
};
pub use confirmation_surface::{
    ProtectedConfirmationView, ProtectedPaymentError, ProtectedPaymentErrorCode,
    ProtectedPaymentSession,
};
pub use diagnostic::SafeDiagnostic;
pub use failure_surface::{FailureSurface, PluginRecovery, RecoveryError, RestartTrigger};
pub use focus_view::{
    FocusOpenError, FocusSelectionError, FocusView, FocusViewModel, FocusViewSnapshot,
    FocusViewState,
};
pub use host::StudioHost;
pub use identity_shell::{IdentityShellRoute, IdentityShellState};
pub use preferences::HostPreferences;
pub use print_preview::PrintPreviewSurface;
pub use resilience::{
    ResilienceCenter, ResilienceEntryPoint, ResilienceRoute, ResilienceRouteError,
};
pub use router::CheckoutRouter;
pub use settings::{
    AboutInfo, AccessibilitySettings, AutosaveSettings, BuildSigningSettings, CapabilitySettings,
    DataSettings, DiagnosticConsent, ExtensionSettings, FeedbackConsent, FeedbackDraft,
    FeedbackError, FeedbackPayload, FeedbackService, FeedbackTransport, GlobalSettingChange,
    GlobalSettings, HelpArticle, HelpNavigator, HelpTopic, InMemorySettingsPersistence,
    KeyboardAction, KeyboardSettings, LanguagePreference, LicenseNotice, MetadataSettings,
    NavigationResult, NotificationSettings, ProjectSettingChange, ProjectSettings,
    RecoverySettings, RedactedDiagnostic, RuntimeIdentitySettings, SETTINGS_SCHEMA_VERSION,
    SettingsController, SettingsEffect, SettingsError, SettingsErrorCode, SettingsPersistence,
    StartupMode, StartupSettings, StorageMode, StorageSyncSettings, SupportNavigator,
    SupportSurface, ThemePreference, UpdateChannel,
};
pub use shutdown::{ShutdownCoordinator, ShutdownReport, ShutdownStep};
