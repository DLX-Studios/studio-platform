//! Bounded owned-message queue used to defer guest emissions.

use std::collections::VecDeque;

use crate::{AbiError, AbiLimits};

#[derive(Debug)]
pub(crate) struct EmissionQueue {
    messages: VecDeque<Vec<u8>>,
    queued_bytes: usize,
    limits: AbiLimits,
}

impl EmissionQueue {
    pub(crate) fn new(limits: AbiLimits) -> Self {
        Self {
            messages: VecDeque::new(),
            queued_bytes: 0,
            limits,
        }
    }

    pub(crate) fn push(&mut self, message: Vec<u8>) -> Result<(), AbiError> {
        let Some(next_bytes) = self.queued_bytes.checked_add(message.len()) else {
            return Err(AbiError::QueueFull);
        };
        if self.messages.len() >= self.limits.max_queued_messages
            || next_bytes > self.limits.max_queued_bytes
        {
            return Err(AbiError::QueueFull);
        }
        self.queued_bytes = next_bytes;
        self.messages.push_back(message);
        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<Vec<u8>> {
        let message = self.messages.pop_front()?;
        self.queued_bytes -= message.len();
        Some(message)
    }

    pub(crate) const fn limits(&self) -> AbiLimits {
        self.limits
    }
}
