//! Wasmtime sandbox, ABI, resource-budget, and plugin-lifecycle boundaries.

mod abi;
mod diagnostic;
mod engine;
mod instance;
mod limits;
mod memory;
mod policy;
mod queue;

pub use abi::{AbiError, AbiErrorCode, AbiLimits, EmitBridge};
pub use diagnostic::GuestDiagnostic;
pub use engine::{EngineError, SandboxEngine};
pub use instance::{
    CallOutcome, InstanceLifecycle, PluginInstance, RuntimeError, RuntimeErrorCode,
};
pub use limits::RuntimeBudgets;
pub use memory::{copy_bytes_from_guest, copy_bytes_to_guest, copy_utf8_from_guest};
pub use policy::{ModulePolicy, PolicyError, PolicyErrorCode, ValidatedModule};
