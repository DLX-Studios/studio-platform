//! Studio Runtime's Wayland-only guest host and retained UI surfaces.

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
pub mod cli;
mod confirmation_surface;
mod diagnostic;
mod failure_surface;
pub mod foundation;
pub mod host;
pub mod plugin_surface;
mod preferences;
mod print_preview;
mod router;
mod shutdown;

pub use action_dispatch::{NativeCheckoutError, NativeCheckoutShell};
pub use confirmation_surface::{
    ProtectedConfirmationView, ProtectedPaymentError, ProtectedPaymentErrorCode,
    ProtectedPaymentSession,
};
pub use diagnostic::SafeDiagnostic;
pub use failure_surface::{FailureSurface, PluginRecovery, RecoveryError, RestartTrigger};
pub use host::StudioHost;
pub use preferences::HostPreferences;
pub use print_preview::PrintPreviewSurface;
pub use router::CheckoutRouter;
pub use shutdown::{ShutdownCoordinator, ShutdownReport, ShutdownStep};
