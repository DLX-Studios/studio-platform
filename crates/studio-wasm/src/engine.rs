//! Pinned, synchronous Wasmtime engine configuration for untrusted core modules.

use thiserror::Error;
use wasmtime::{Config, Engine};

/// Wasmtime engine configured with Studio's closed proposal policy.
#[derive(Clone)]
pub struct SandboxEngine {
    pub(crate) inner: Engine,
}

impl SandboxEngine {
    /// Build the deterministic Studio core-WebAssembly engine.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when Wasmtime rejects the host configuration.
    pub fn new() -> Result<Self, EngineError> {
        let mut config = Config::new();
        config
            .consume_fuel(true)
            .epoch_interruption(true)
            .cranelift_nan_canonicalization(true)
            .wasm_simd(false)
            .wasm_relaxed_simd(false)
            .wasm_tail_call(false)
            .wasm_wide_arithmetic(false)
            .wasm_bulk_memory(false)
            .wasm_multi_value(false)
            .wasm_multi_memory(false)
            .wasm_memory64(false)
            .wasm_custom_page_sizes(false)
            .wasm_extended_const(false)
            .wasm_stack_switching(false);

        Engine::new(&config)
            .map(|inner| Self { inner })
            .map_err(|error| EngineError::Configuration(error.to_string()))
    }
}

/// Sandbox-engine construction failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EngineError {
    /// Wasmtime rejected Studio's explicit engine configuration.
    #[error("invalid Studio Wasmtime configuration: {0}")]
    Configuration(String),
}
