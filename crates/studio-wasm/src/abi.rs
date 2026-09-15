//! Non-reentrant `studio_host.emit` bridge and stable ABI failures.

use thiserror::Error;

use crate::{memory::copy_bytes_from_guest, queue::EmissionQueue};

/// Host-side copy and queue budgets for the guest ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbiLimits {
    /// Maximum bytes copied by one `emit` call.
    pub max_message_bytes: usize,
    /// Maximum owned messages awaiting host processing.
    pub max_queued_messages: usize,
    /// Maximum total owned bytes awaiting host processing.
    pub max_queued_bytes: usize,
}

impl Default for AbiLimits {
    fn default() -> Self {
        Self {
            max_message_bytes: 1024 * 1024,
            max_queued_messages: 64,
            max_queued_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Owned-message bridge that never synchronously re-enters an active guest call.
#[derive(Debug)]
pub struct EmitBridge {
    queue: EmissionQueue,
    guest_call_active: bool,
}

impl EmitBridge {
    /// Create an empty bridge with fixed queue limits.
    #[must_use]
    pub fn new(limits: AbiLimits) -> Self {
        Self {
            queue: EmissionQueue::new(limits),
            guest_call_active: false,
        }
    }

    /// Enter one `studio_init` or `studio_event` guest call.
    ///
    /// # Errors
    ///
    /// Returns [`AbiError::ReentrantCall`] if a guest call is already active.
    pub fn begin_guest_call(&mut self) -> Result<(), AbiError> {
        if self.guest_call_active {
            return Err(AbiError::ReentrantCall);
        }
        self.guest_call_active = true;
        Ok(())
    }

    /// Mark the active guest call returned.
    ///
    /// # Errors
    ///
    /// Returns [`AbiError::CallStateInvalid`] if no guest call is active.
    pub fn end_guest_call(&mut self) -> Result<(), AbiError> {
        if !self.guest_call_active {
            return Err(AbiError::CallStateInvalid);
        }
        self.guest_call_active = false;
        Ok(())
    }

    /// Copy one guest emission into the bounded deferred queue.
    ///
    /// # Errors
    ///
    /// Returns [`AbiError`] for call-state, pointer, size, or queue violations.
    pub fn emit(&mut self, memory: &[u8], pointer: i32, length: i32) -> Result<(), AbiError> {
        if !self.guest_call_active {
            return Err(AbiError::CallStateInvalid);
        }
        let message = copy_bytes_from_guest(
            memory,
            pointer,
            length,
            self.queue.limits().max_message_bytes,
        )?;
        self.queue.push(message)
    }

    /// Pop one ready message only after the guest call has returned.
    pub fn pop_ready(&mut self) -> Option<Vec<u8>> {
        if self.guest_call_active {
            None
        } else {
            self.queue.pop()
        }
    }

    pub(crate) fn enqueue_owned(&mut self, message: Vec<u8>) -> Result<(), AbiError> {
        if !self.guest_call_active {
            return Err(AbiError::CallStateInvalid);
        }
        self.queue.push(message)
    }

    pub(crate) const fn maximum_message_bytes(&self) -> usize {
        self.queue.limits().max_message_bytes
    }
}

/// Stable ABI error family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiErrorCode {
    /// Pointer/length arithmetic or bounds were invalid.
    PointerInvalid,
    /// A copied message exceeded its fixed byte budget.
    MessageTooLarge,
    /// Copied bytes were not valid UTF-8.
    Utf8Invalid,
    /// The deferred emission queue reached a fixed budget.
    QueueFull,
    /// The host attempted to synchronously re-enter a guest call.
    ReentrantCall,
    /// An ABI operation was invoked outside its required call state.
    CallStateInvalid,
}

/// Detailed checked-ABI failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AbiError {
    /// Invalid wasm32 pointer/length pair.
    #[error("invalid guest range ptr={pointer} len={length} for {memory_bytes} bytes")]
    PointerInvalid {
        /// Raw signed ABI pointer.
        pointer: i32,
        /// Raw signed ABI length.
        length: i32,
        /// Current linear-memory size.
        memory_bytes: usize,
    },
    /// Message byte budget exceeded.
    #[error("guest message is {actual} bytes; limit is {limit}")]
    MessageTooLarge {
        /// Actual copied range length.
        actual: usize,
        /// Fixed maximum length.
        limit: usize,
    },
    /// Copied bytes were invalid UTF-8.
    #[error("guest message is not valid UTF-8")]
    Utf8Invalid,
    /// Queue count or byte budget exceeded.
    #[error("guest emission queue is full")]
    QueueFull,
    /// A guest call was already active.
    #[error("synchronous guest re-entry is forbidden")]
    ReentrantCall,
    /// The ABI operation was invoked in the wrong call state.
    #[error("ABI call state is invalid")]
    CallStateInvalid,
}

impl AbiError {
    /// Return the stable family for this detailed ABI failure.
    #[must_use]
    pub const fn code(&self) -> AbiErrorCode {
        match self {
            Self::PointerInvalid { .. } => AbiErrorCode::PointerInvalid,
            Self::MessageTooLarge { .. } => AbiErrorCode::MessageTooLarge,
            Self::Utf8Invalid => AbiErrorCode::Utf8Invalid,
            Self::QueueFull => AbiErrorCode::QueueFull,
            Self::ReentrantCall => AbiErrorCode::ReentrantCall,
            Self::CallStateInvalid => AbiErrorCode::CallStateInvalid,
        }
    }
}
