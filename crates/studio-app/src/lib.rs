//! Studio's Wayland-only native shell and host-owned foundation surfaces.

mod action_dispatch;
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
