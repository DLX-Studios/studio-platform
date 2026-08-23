//! Wasmtime store ownership, host linking, call budgets, and terminal lifecycle.

use std::{fmt, sync::mpsc, thread};

use thiserror::Error;
use wasmtime::{
    Caller, Extern, Instance, Linker, Memory, Store, StoreLimits, StoreLimitsBuilder, Trap,
    TypedFunc,
};

use crate::{
    AbiError, EmitBridge, RuntimeBudgets, SandboxEngine, ValidatedModule, copy_bytes_from_guest,
};

#[derive(Debug)]
struct StoreData {
    limits: StoreLimits,
    bridge: EmitBridge,
    abi_violation: Option<AbiError>,
}

/// Host-observed lifecycle for one isolated plugin instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceLifecycle {
    /// The module is instantiated and accepts calls.
    Running,
    /// A trap, resource excess, or protocol violation permanently ended the instance.
    Terminated,
}

/// One isolated Wasmtime store and its fixed host-owned resources.
pub struct PluginInstance {
    engine: SandboxEngine,
    store: Store<StoreData>,
    _instance: Instance,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    dealloc: TypedFunc<(i32, i32), ()>,
    init: TypedFunc<(i32, i32), i32>,
    event: TypedFunc<(i32, i32), i32>,
    budgets: RuntimeBudgets,
    lifecycle: InstanceLifecycle,
}

impl fmt::Debug for PluginInstance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginInstance")
            .field("budgets", &self.budgets)
            .field("lifecycle", &self.lifecycle)
            .finish_non_exhaustive()
    }
}

impl PluginInstance {
    /// Instantiate a policy-validated module with the sole `studio_host.emit` import.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] if store limits, linking, instantiation, or typed exports fail.
    pub fn instantiate(
        engine: SandboxEngine,
        validated: ValidatedModule,
        budgets: RuntimeBudgets,
    ) -> Result<Self, RuntimeError> {
        let module = validated.into_module();
        let limits = StoreLimitsBuilder::new()
            .memory_size(budgets.max_memory_bytes)
            .table_elements(budgets.max_table_elements)
            .instances(1)
            .memories(1)
            .tables(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &engine.inner,
            StoreData {
                limits,
                bridge: EmitBridge::new(budgets.abi_limits),
                abi_violation: None,
            },
        );
        store.limiter(|data| &mut data.limits);
        store
            .set_fuel(budgets.initialization_fuel)
            .map_err(|error| RuntimeError::ResourceExhausted(error.to_string()))?;
        store.set_epoch_deadline(1);

        let mut linker = Linker::new(&engine.inner);
        linker
            .func_wrap(
                "studio_host",
                "emit",
                |mut caller: Caller<'_, StoreData>, pointer: i32, length: i32| -> i32 {
                    let copied = copy_emission(&mut caller, pointer, length);
                    let result =
                        copied.and_then(|message| caller.data_mut().bridge.enqueue_owned(message));
                    match result {
                        Ok(()) => 0,
                        Err(error) => {
                            caller.data_mut().abi_violation = Some(error);
                            1
                        }
                    }
                },
            )
            .map_err(|error| RuntimeError::Instantiation(error.to_string()))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| RuntimeError::ResourceExhausted(error.to_string()))?;
        let init = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "studio_init")
            .map_err(|error| RuntimeError::Instantiation(error.to_string()))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| RuntimeError::Instantiation("memory export is missing".to_owned()))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "studio_alloc")
            .map_err(|error| RuntimeError::Instantiation(error.to_string()))?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "studio_dealloc")
            .map_err(|error| RuntimeError::Instantiation(error.to_string()))?;
        let event = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "studio_event")
            .map_err(|error| RuntimeError::Instantiation(error.to_string()))?;

        Ok(Self {
            engine,
            store,
            _instance: instance,
            memory,
            alloc,
            dealloc,
            init,
            event,
            budgets,
            lifecycle: InstanceLifecycle::Running,
        })
    }

    /// Return the current host-owned lifecycle.
    #[must_use]
    pub const fn lifecycle(&self) -> InstanceLifecycle {
        self.lifecycle
    }

    /// Invoke `studio_init` under fresh fuel and epoch budgets.
    ///
    /// # Errors
    ///
    /// Returns a terminal [`RuntimeError`] for ABI failures, traps, or resource exhaustion.
    pub fn invoke_init(&mut self, pointer: i32, length: i32) -> Result<CallOutcome, RuntimeError> {
        let function = self.init.clone();
        self.invoke_with_fuel(&function, pointer, length, self.budgets.initialization_fuel)
    }

    /// Invoke `studio_event` under fresh fuel and epoch budgets.
    ///
    /// # Errors
    ///
    /// Returns a terminal [`RuntimeError`] for ABI failures, traps, or resource exhaustion.
    pub fn invoke_event(&mut self, pointer: i32, length: i32) -> Result<CallOutcome, RuntimeError> {
        let function = self.event.clone();
        self.invoke_with_fuel(&function, pointer, length, self.budgets.fuel_per_call)
    }

    /// Copy one owned host event into guest memory and invoke `studio_event`.
    ///
    /// # Errors
    ///
    /// Returns a terminal [`RuntimeError`] for size, allocation, memory, ABI, or guest failures.
    pub fn invoke_event_bytes(&mut self, bytes: &[u8]) -> Result<CallOutcome, RuntimeError> {
        if self.lifecycle == InstanceLifecycle::Terminated {
            return Err(RuntimeError::GuestTerminated);
        }
        let length = i32::try_from(bytes.len()).map_err(|_| {
            self.terminate(RuntimeError::AbiInvalid(
                "host event is too large".to_owned(),
            ))
        })?;
        self.store
            .set_fuel(self.budgets.fuel_per_call)
            .map_err(|error| self.terminate(RuntimeError::ResourceExhausted(error.to_string())))?;
        let pointer = self
            .alloc
            .call(&mut self.store, length)
            .map_err(|error| self.terminate(classify_trap(&error)))?;
        if let Some(error) = self.store.data_mut().abi_violation.take() {
            return Err(self.terminate(RuntimeError::AbiInvalid(error.to_string())));
        }
        let Ok(pointer_usize) = usize::try_from(pointer) else {
            return Err(self.terminate(RuntimeError::AbiInvalid(
                "guest allocation pointer is negative".to_owned(),
            )));
        };
        self.memory
            .write(&mut self.store, pointer_usize, bytes)
            .map_err(|error| self.terminate(RuntimeError::AbiInvalid(error.to_string())))?;
        let outcome = self.invoke_event(pointer, length)?;
        let dealloc = self.dealloc.clone();
        dealloc
            .call(&mut self.store, (pointer, length))
            .map_err(|error| self.terminate(classify_trap(&error)))?;
        Ok(outcome)
    }

    fn invoke_with_fuel(
        &mut self,
        function: &TypedFunc<(i32, i32), i32>,
        pointer: i32,
        length: i32,
        fuel: u64,
    ) -> Result<CallOutcome, RuntimeError> {
        if self.lifecycle == InstanceLifecycle::Terminated {
            return Err(RuntimeError::GuestTerminated);
        }
        self.store
            .set_fuel(fuel)
            .map_err(|error| self.terminate(RuntimeError::ResourceExhausted(error.to_string())))?;
        self.store.set_epoch_deadline(1);
        self.store
            .data_mut()
            .bridge
            .begin_guest_call()
            .map_err(|error| self.terminate(RuntimeError::AbiInvalid(error.to_string())))?;

        let (cancel_sender, cancel_receiver) = mpsc::sync_channel::<()>(1);
        let deadline = self.budgets.call_deadline;
        let deadline_engine = self.engine.inner.clone();
        let timer = thread::spawn(move || {
            if matches!(
                cancel_receiver.recv_timeout(deadline),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                deadline_engine.increment_epoch();
            }
        });

        let result = function.call(&mut self.store, (pointer, length));
        let _ = cancel_sender.send(());
        let _ = timer.join();
        let end_result = self.store.data_mut().bridge.end_guest_call();
        if let Err(error) = end_result {
            return Err(self.terminate(RuntimeError::AbiInvalid(error.to_string())));
        }
        if let Some(error) = self.store.data_mut().abi_violation.take() {
            return Err(self.terminate(RuntimeError::AbiInvalid(error.to_string())));
        }

        let status = match result {
            Ok(status) => status,
            Err(error) => return Err(self.terminate(classify_trap(&error))),
        };
        if status != 0 {
            return Err(self.terminate(RuntimeError::AbiInvalid(format!(
                "guest returned ABI status {status}"
            ))));
        }
        let mut emissions = Vec::new();
        while let Some(message) = self.store.data_mut().bridge.pop_ready() {
            emissions.push(message);
        }
        Ok(CallOutcome { status, emissions })
    }

    fn terminate(&mut self, error: RuntimeError) -> RuntimeError {
        self.lifecycle = InstanceLifecycle::Terminated;
        error
    }
}

fn copy_emission(
    caller: &mut Caller<'_, StoreData>,
    pointer: i32,
    length: i32,
) -> Result<Vec<u8>, AbiError> {
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        return Err(AbiError::CallStateInvalid);
    };
    copy_from_memory(memory, caller, pointer, length)
}

fn copy_from_memory(
    memory: Memory,
    caller: &Caller<'_, StoreData>,
    pointer: i32,
    length: i32,
) -> Result<Vec<u8>, AbiError> {
    copy_bytes_from_guest(
        memory.data(caller),
        pointer,
        length,
        caller.data().bridge.maximum_message_bytes(),
    )
}

fn classify_trap(error: &wasmtime::Error) -> RuntimeError {
    match error.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel | Trap::Interrupt) => {
            RuntimeError::ResourceExhausted(error.to_string())
        }
        _ => RuntimeError::GuestTrapped(error.to_string()),
    }
}

/// Successful guest call output after deferred emissions become ready.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOutcome {
    /// Zero ABI status returned by the guest.
    pub status: i32,
    /// Ordered, owned messages emitted during the completed call.
    pub emissions: Vec<Vec<u8>>,
}

/// Stable runtime failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeErrorCode {
    /// Store, fuel, epoch, memory, table, or queue resource exhaustion.
    ResourceExhausted,
    /// Guest executed a non-resource trap.
    GuestTrapped,
    /// Guest violated the ABI boundary.
    AbiInvalid,
    /// Linker, instance, or typed-export construction failed.
    InstantiationFailed,
    /// A prior terminal failure makes further calls illegal.
    GuestTerminated,
}

/// Detailed plugin instance failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeError {
    /// Store or runtime resource ceiling reached.
    #[error("runtime resource exhausted: {0}")]
    ResourceExhausted(String),
    /// Guest trapped for a non-resource reason.
    #[error("guest trapped: {0}")]
    GuestTrapped(String),
    /// Guest violated a checked ABI rule.
    #[error("guest ABI invalid: {0}")]
    AbiInvalid(String),
    /// Linking or instantiation failed.
    #[error("plugin instantiation failed: {0}")]
    Instantiation(String),
    /// Instance was already terminal.
    #[error("guest instance is terminated")]
    GuestTerminated,
}

impl RuntimeError {
    /// Return the stable family for this detailed runtime failure.
    #[must_use]
    pub const fn code(&self) -> RuntimeErrorCode {
        match self {
            Self::ResourceExhausted(_) => RuntimeErrorCode::ResourceExhausted,
            Self::GuestTrapped(_) => RuntimeErrorCode::GuestTrapped,
            Self::AbiInvalid(_) => RuntimeErrorCode::AbiInvalid,
            Self::Instantiation(_) => RuntimeErrorCode::InstantiationFailed,
            Self::GuestTerminated => RuntimeErrorCode::GuestTerminated,
        }
    }

    /// Stable host-surface code that excludes trap text and guest-controlled context.
    #[must_use]
    pub const fn safe_failure_code(&self) -> &'static str {
        match self.code() {
            RuntimeErrorCode::ResourceExhausted => "resource_exhausted",
            RuntimeErrorCode::GuestTrapped => "guest_trapped",
            RuntimeErrorCode::AbiInvalid => "abi_invalid",
            RuntimeErrorCode::InstantiationFailed => "instantiation_failed",
            RuntimeErrorCode::GuestTerminated => "guest_terminated",
        }
    }
}
