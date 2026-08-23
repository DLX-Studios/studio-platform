//! Fixed runtime ceilings applied to each plugin instance and guest call.

use std::time::Duration;

use crate::AbiLimits;

/// Host ceilings intersected with any stricter manifest request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBudgets {
    /// Maximum bytes in the instance's single linear memory.
    pub max_memory_bytes: usize,
    /// Maximum elements in the instance's single table.
    pub max_table_elements: usize,
    /// Fuel available while instantiating the module and running `studio_init`.
    pub initialization_fuel: u64,
    /// Fuel restored before every event call.
    pub fuel_per_call: u64,
    /// Wall-clock deadline enforced through Wasmtime epochs.
    pub call_deadline: Duration,
    /// Copy and deferred-emission limits.
    pub abi_limits: AbiLimits,
}

impl Default for RuntimeBudgets {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024,
            max_table_elements: 1024,
            initialization_fuel: 15_000_000,
            fuel_per_call: 10_000_000,
            call_deadline: Duration::from_millis(50),
            abi_limits: AbiLimits::default(),
        }
    }
}
