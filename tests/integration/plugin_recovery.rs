#![allow(missing_docs)]

use studio_app::{PluginRecovery, RestartTrigger};
use studio_security::{PluginPrincipal, SecretPurpose, TrustMode};
use studio_wasm::{ModulePolicy, PluginInstance, RuntimeBudgets, RuntimeErrorCode, SandboxEngine};

fn principal(instance: u8) -> PluginPrincipal {
    PluginPrincipal::new(
        "publisher",
        "plugin",
        [1; 32],
        [instance; 16],
        TrustMode::Production,
    )
    .unwrap()
}

fn trapping_instance() -> PluginInstance {
    let bytes = wat::parse_str(
        r#"(module
      (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
      (memory (export "memory") 1 256) (table 0 1024 funcref)
      (func (export "studio_alloc") (param i32) (result i32) i32.const 0)
      (func (export "studio_dealloc") (param i32 i32))
      (func (export "studio_init") (param i32 i32) (result i32) i32.const 0)
      (func (export "studio_event") (param i32 i32) (result i32) unreachable))"#,
    )
    .unwrap();
    let engine = SandboxEngine::new().unwrap();
    let module = ModulePolicy::default().validate(&engine, &bytes).unwrap();
    PluginInstance::instantiate(engine, module, RuntimeBudgets::default()).unwrap()
}

#[test]
fn trap_terminates_owned_state_and_only_manual_restart_creates_a_fresh_instance() {
    let owner = principal(1);
    let mut recovery = PluginRecovery::new(owner.clone()).unwrap();
    let old_id = recovery.instance_id();
    recovery.mount_ui();
    recovery.set_plugin_state("counter", "9");
    recovery.begin_action("payment-1").unwrap();
    recovery
        .capture_secret(SecretPurpose::PaymentPin, "sale-1", b"8642")
        .unwrap();

    let mut guest = trapping_instance();
    let error = guest.invoke_event(0, 0).unwrap_err();
    assert_eq!(error.code(), RuntimeErrorCode::GuestTrapped);
    recovery.terminate(&error);
    assert!(recovery.host_responsive());
    assert_eq!(recovery.failure_surface().unwrap().code(), "guest_trapped");
    assert_eq!(recovery.active_secrets(), 0);
    assert_eq!(recovery.pending_actions(), 0);
    assert!(!recovery.ui_mounted());
    assert_eq!(recovery.plugin_state("counter"), None);

    assert!(recovery.restart(RestartTrigger::Automatic).is_err());
    recovery.restart(RestartTrigger::Manual).unwrap();
    assert_ne!(recovery.instance_id(), old_id);
    assert!(recovery.failure_surface().is_none());
    assert_eq!(recovery.plugin_state("counter"), None);
}

#[test]
fn resource_exhaustion_uses_the_same_host_owned_recovery_surface() {
    let mut recovery = PluginRecovery::new(principal(2)).unwrap();
    let error = studio_wasm::RuntimeError::ResourceExhausted("secret raw context".to_owned());
    recovery.terminate(&error);
    assert_eq!(
        recovery.failure_surface().unwrap().code(),
        "resource_exhausted"
    );
    assert!(
        !recovery
            .failure_surface()
            .unwrap()
            .message()
            .contains("secret raw context")
    );
}
