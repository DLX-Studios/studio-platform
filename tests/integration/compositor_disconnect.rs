#![allow(missing_docs)]

use studio_app::{ShutdownCoordinator, ShutdownStep};
use studio_security::{PluginPrincipal, SecretPurpose, TrustMode};

fn principal() -> PluginPrincipal {
    PluginPrincipal::new("publisher", "pos", [1; 32], [2; 16], TrustMode::Production).unwrap()
}

#[test]
fn compositor_loss_performs_ordered_terminal_cleanup_without_restoration() {
    let mut shutdown = ShutdownCoordinator::new(principal());
    shutdown.start_instance();
    shutdown.mount_ui_and_navigation();
    shutdown.begin_payment("payment-1").unwrap();
    shutdown
        .capture_secret(SecretPurpose::PaymentPin, "sale-1", b"8642")
        .unwrap();
    assert!(shutdown.has_live_resources());

    let report = shutdown.compositor_lost();
    assert_eq!(
        report.steps(),
        &[
            ShutdownStep::ActionsCancelled,
            ShutdownStep::SecretsRevoked,
            ShutdownStep::InstanceTerminated,
            ShutdownStep::NativeStateClosed,
            ShutdownStep::ProcessExitRequested,
        ]
    );
    assert_eq!(report.cancelled_actions(), 1);
    assert_eq!(report.revoked_secrets(), 1);
    assert!(!shutdown.has_live_resources());
    assert!(shutdown.exit_requested());
    assert!(shutdown.restore().is_err());
}
