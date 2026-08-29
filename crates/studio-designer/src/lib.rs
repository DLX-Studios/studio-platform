//! Studio Designer's native product shell and session-backed editing surfaces.
//!
//! This crate is a separate application from `studio-app`: it owns Designer
//! identity, dashboard, persistence, navigation, and Focus presentation while
//! Runtime bundle admission remains in the Runtime application.

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
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::semicolon_if_nothing_returned,
    clippy::needless_pass_by_value
)]

pub mod bootstrap;
pub mod connection;
mod diagnostic;
pub mod focus_view;
pub mod identity_shell;
pub mod project_dashboard;
pub mod resilience;
pub mod settings;
mod shell_theme;
mod welcome;

pub use bootstrap::{
    BootstrapError, LocalStoreDashboardPersistence, LocalStoreSettingsPersistence,
    NativeProductBootstrap, NativeProductShell, NativeProductState, OfflineSyncWorker,
    ProductRoute,
};
pub use diagnostic::SafeDiagnostic;
pub use focus_view::{
    FocusOpenError, FocusSelectionError, FocusView, FocusViewModel, FocusViewSnapshot,
    FocusViewState,
};
pub use identity_shell::{IdentityShellRoute, IdentityShellState};
pub use resilience::{
    ResilienceCenter, ResilienceEntryPoint, ResilienceRoute, ResilienceRouteError,
};
pub use settings::*;
pub mod agent_adapter;
pub mod content_editors;
pub mod conversation_composer;
pub mod library_panel;
pub mod mcp_surface;
pub mod plugin_template_ux;
