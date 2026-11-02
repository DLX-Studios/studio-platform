#![cfg(feature = "development")]
//! Development hooks are optional, bounded and excluded by production module policy.
use studio_wasm::{ModulePolicy, PluginInstance, RuntimeBudgets, SandboxEngine};

fn module() -> Vec<u8> {
    wat::parse_str(
        r#"(module
      (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
      (memory (export "memory") 1 256)
      (table 0 1024 funcref)
      (data (i32.const 0) "state:1")
      (global $pointer (mut i32) (i32.const 0))
      (global $length (mut i32) (i32.const 7))
      (func (export "studio_alloc") (param i32) (result i32) i32.const 2048)
      (func (export "studio_dealloc") (param i32 i32))
      (func (export "studio_init") (param i32 i32) (result i32) i32.const 0)
      (func (export "studio_event") (param i32 i32) (result i32)
        i32.const 6 i32.const 50 i32.store8 i32.const 0)
      (func (export "studio_dev_snapshot") (param i32 i32) (result i32)
        global.get $pointer global.get $length call $emit)
      (func (export "studio_dev_restore") (param i32 i32) (result i32)
        local.get 0 global.set $pointer local.get 1 global.set $length i32.const 0)
    )"#,
    )
    .unwrap()
}

#[test]
fn production_rejects_dev_exports_and_hot_state_transfers_to_an_isolated_new_guest() {
    let bytes = module();
    let engine = SandboxEngine::new().unwrap();
    let policy = ModulePolicy::default();
    assert!(policy.validate(&engine, &bytes).is_err());
    let validated = policy.validate_development(&engine, &bytes).unwrap();
    let mut live =
        PluginInstance::instantiate(engine.clone(), validated, RuntimeBudgets::default()).unwrap();
    live.invoke_init(0, 0).unwrap();
    live.invoke_event(0, 0).unwrap();
    let snapshot = live.snapshot_development_state().unwrap().unwrap();
    assert_eq!(snapshot, b"state:2");
    let next = policy.validate_development(&engine, &bytes).unwrap();
    let mut candidate =
        PluginInstance::instantiate(engine, next, RuntimeBudgets::default()).unwrap();
    assert!(
        candidate
            .restore_development_state(&vec![0; 65_537])
            .is_err()
    );
    assert_eq!(
        candidate.snapshot_development_state().unwrap().unwrap(),
        b"state:1"
    );
    candidate.restore_development_state(&snapshot).unwrap();
    assert_eq!(
        candidate.snapshot_development_state().unwrap().unwrap(),
        b"state:2"
    );
    assert_eq!(
        live.snapshot_development_state().unwrap().unwrap(),
        b"state:2"
    );
}

#[test]
fn development_policy_rejects_unpaired_and_wrongly_typed_hooks() {
    let bytes = module();
    let engine = SandboxEngine::new().unwrap();
    assert!(
        ModulePolicy::default()
            .validate_development(&engine, &bytes)
            .is_ok()
    );
    let source = r#"(module
      (import "studio_host" "emit" (func (param i32 i32) (result i32)))
      (memory (export "memory") 1 256)
      (table 0 1024 funcref)
      (func (export "studio_alloc") (param i32) (result i32) i32.const 0)
      (func (export "studio_dealloc") (param i32 i32))
      (func (export "studio_init") (param i32 i32) (result i32) i32.const 0)
      (func (export "studio_event") (param i32 i32) (result i32) i32.const 0)
      (func (export "studio_dev_snapshot") (param i32 i32) (result i32) i32.const 0)
    )"#;
    let incomplete = wat::parse_str(source).unwrap();
    assert!(
        ModulePolicy::default()
            .validate_development(&engine, &incomplete)
            .is_err()
    );
}
