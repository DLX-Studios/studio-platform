#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_wasm::{
    AbiLimits, InstanceLifecycle, ModulePolicy, PluginInstance, RuntimeBudgets, RuntimeErrorCode,
    SandboxEngine,
};

fn module(event_body: &str, table_minimum: u32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
          (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
          (memory $memory 1 256)
          (export "memory" (memory $memory))
          (table {table_minimum} 1024 funcref)
          (func $studio_alloc (param i32) (result i32) unreachable)
          (export "studio_alloc" (func $studio_alloc))
          (func $studio_dealloc (param i32 i32) unreachable)
          (export "studio_dealloc" (func $studio_dealloc))
          (func $studio_init (param i32 i32) (result i32) i32.const 0)
          (export "studio_init" (func $studio_init))
          (func $studio_event (param i32 i32) (result i32) {event_body})
          (export "studio_event" (func $studio_event))
        )"#
    ))
    .unwrap()
}

fn instantiate(
    event_body: &str,
    budgets: RuntimeBudgets,
) -> Result<PluginInstance, studio_wasm::RuntimeError> {
    let engine = SandboxEngine::new().unwrap();
    let validated = ModulePolicy::default()
        .validate(&engine, &module(event_body, 0))
        .unwrap();
    PluginInstance::instantiate(engine, validated, budgets)
}

#[test]
fn default_runtime_budgets_match_the_v1_contract() {
    let budgets = RuntimeBudgets::default();
    assert_eq!(budgets.max_memory_bytes, 16 * 1024 * 1024);
    assert_eq!(budgets.max_table_elements, 1024);
    assert_eq!(budgets.initialization_fuel, 15_000_000);
    assert_eq!(budgets.fuel_per_call, 10_000_000);
    assert_eq!(budgets.call_deadline, Duration::from_millis(50));
}

#[test]
fn resets_fuel_for_every_successful_guest_call() {
    let counted_loop = r"
      (local $remaining i32)
      i32.const 100
      local.set $remaining
      (block $done
        (loop $again
          local.get $remaining
          i32.eqz
          br_if $done
          local.get $remaining
          i32.const 1
          i32.sub
          local.set $remaining
          br $again))
      i32.const 0
    ";
    let budgets = RuntimeBudgets {
        fuel_per_call: 2_000,
        call_deadline: Duration::from_secs(1),
        ..RuntimeBudgets::default()
    };
    let mut instance = instantiate(counted_loop, budgets).unwrap();

    instance.invoke_event(0, 0).unwrap();
    instance.invoke_event(0, 0).unwrap();
    assert_eq!(instance.lifecycle(), InstanceLifecycle::Running);
}

#[test]
fn fuel_exhaustion_terminates_and_terminal_instances_never_run_again() {
    let budgets = RuntimeBudgets {
        fuel_per_call: 1_000,
        call_deadline: Duration::from_secs(1),
        ..RuntimeBudgets::default()
    };
    let mut instance = instantiate("(loop $spin (br $spin)) unreachable", budgets).unwrap();

    assert_eq!(
        instance.invoke_event(0, 0).unwrap_err().code(),
        RuntimeErrorCode::ResourceExhausted
    );
    assert_eq!(instance.lifecycle(), InstanceLifecycle::Terminated);
    assert_eq!(
        instance.invoke_event(0, 0).unwrap_err().code(),
        RuntimeErrorCode::GuestTerminated
    );
}

#[test]
fn epoch_deadline_interrupts_long_running_guest_work() {
    let budgets = RuntimeBudgets {
        fuel_per_call: u64::MAX,
        call_deadline: Duration::from_millis(50),
        ..RuntimeBudgets::default()
    };
    let mut instance = instantiate("(loop $spin (br $spin)) unreachable", budgets).unwrap();
    let started = Instant::now();

    assert_eq!(
        instance.invoke_event(0, 0).unwrap_err().code(),
        RuntimeErrorCode::ResourceExhausted
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(instance.lifecycle(), InstanceLifecycle::Terminated);
}

#[test]
fn traps_and_emit_queue_violations_are_terminal() {
    let mut trapped = instantiate("unreachable", RuntimeBudgets::default()).unwrap();
    assert_eq!(
        trapped.invoke_event(0, 0).unwrap_err().code(),
        RuntimeErrorCode::GuestTrapped
    );
    assert_eq!(trapped.lifecycle(), InstanceLifecycle::Terminated);

    let budgets = RuntimeBudgets {
        abi_limits: AbiLimits {
            max_message_bytes: 64,
            max_queued_messages: 0,
            max_queued_bytes: 64,
        },
        ..RuntimeBudgets::default()
    };
    let mut queue_violator = instantiate("i32.const 0 i32.const 0 call $emit", budgets).unwrap();
    assert_eq!(
        queue_violator.invoke_event(0, 0).unwrap_err().code(),
        RuntimeErrorCode::AbiInvalid
    );
    assert_eq!(queue_violator.lifecycle(), InstanceLifecycle::Terminated);
}

#[test]
fn store_limits_reject_initial_memory_and_table_resources_above_host_budgets() {
    let engine = SandboxEngine::new().unwrap();
    let bytes = module("i32.const 0", 1);
    let validated = ModulePolicy::default().validate(&engine, &bytes).unwrap();
    let table_error = PluginInstance::instantiate(
        engine,
        validated,
        RuntimeBudgets {
            max_table_elements: 0,
            ..RuntimeBudgets::default()
        },
    )
    .unwrap_err();
    assert_eq!(table_error.code(), RuntimeErrorCode::ResourceExhausted);

    let engine = SandboxEngine::new().unwrap();
    let validated = ModulePolicy::default()
        .validate(&engine, &module("i32.const 0", 0))
        .unwrap();
    let memory_error = PluginInstance::instantiate(
        engine,
        validated,
        RuntimeBudgets {
            max_memory_bytes: 0,
            ..RuntimeBudgets::default()
        },
    )
    .unwrap_err();
    assert_eq!(memory_error.code(), RuntimeErrorCode::ResourceExhausted);
}
