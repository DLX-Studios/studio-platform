//! Process-lifetime terminal payment record retention.

use std::collections::HashMap;

use studio_security::PluginPrincipal;

use crate::{Money, PaymentError, PaymentErrorCode, PaymentResult};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaymentFingerprint {
    owner: PluginPrincipal,
    session_id: String,
    amount: Money,
}

impl PaymentFingerprint {
    pub(crate) fn new(owner: PluginPrincipal, session_id: String, amount: Money) -> Self {
        Self {
            owner,
            session_id,
            amount,
        }
    }
}

pub(crate) enum Lookup {
    Missing,
    Replay(PaymentResult),
}

pub(crate) struct IdempotencyRegistry {
    capacity: usize,
    records: HashMap<String, (PaymentFingerprint, PaymentResult)>,
}

impl IdempotencyRegistry {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            records: HashMap::new(),
        }
    }

    pub(crate) fn lookup(
        &self,
        key: &str,
        fingerprint: &PaymentFingerprint,
    ) -> Result<Lookup, PaymentError> {
        match self.records.get(key) {
            Some((retained, result)) if retained == fingerprint => {
                Ok(Lookup::Replay(result.clone()))
            }
            Some(_) => Err(PaymentError::new(PaymentErrorCode::IdempotencyConflict)),
            None => Ok(Lookup::Missing),
        }
    }

    pub(crate) fn insert(
        &mut self,
        key: String,
        fingerprint: PaymentFingerprint,
        result: PaymentResult,
    ) -> Result<(), PaymentError> {
        if self.records.len() >= self.capacity {
            return Err(PaymentError::new(
                PaymentErrorCode::IdempotencyCapacityExhausted,
            ));
        }
        self.records.insert(key, (fingerprint, result));
        Ok(())
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) const fn capacity(&self) -> usize {
        self.capacity
    }
}
