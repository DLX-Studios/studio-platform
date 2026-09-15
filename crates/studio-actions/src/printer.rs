//! Structured-only, in-memory printer preview simulator.

use std::{collections::HashMap, error::Error, fmt};

use serde::Deserialize;
use studio_security::PluginPrincipal;

use crate::{Receipt, ReceiptLine};

/// Stable printer-simulator failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintErrorCode {
    /// The command was not the closed structured request schema.
    InvalidRequest,
    /// The referenced approved receipt is unavailable.
    ReceiptNotFound,
    /// The principal does not own the receipt.
    OwnerMismatch,
    /// The request identifier has already produced a job.
    DuplicateRequest,
}

/// Safe printer-simulator error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrintError(PrintErrorCode);

impl PrintError {
    const fn new(code: PrintErrorCode) -> Self {
        Self(code)
    }

    /// Stable failure code.
    #[must_use]
    pub const fn code(self) -> PrintErrorCode {
        self.0
    }
}

impl fmt::Display for PrintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            PrintErrorCode::InvalidRequest => "structured print request invalid",
            PrintErrorCode::ReceiptNotFound => "approved receipt not found",
            PrintErrorCode::OwnerMismatch => "receipt owner mismatch",
            PrintErrorCode::DuplicateRequest => "print request duplicate",
        })
    }
}

impl Error for PrintError {}

/// Closed plugin-visible print-preview request.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintPreviewRequest {
    request_id: String,
    receipt_id: String,
}

impl PrintPreviewRequest {
    /// Create a request from bounded identities.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or control-bearing values.
    pub fn new(
        request_id: impl Into<String>,
        receipt_id: impl Into<String>,
    ) -> Result<Self, PrintError> {
        let request = Self {
            request_id: request_id.into(),
            receipt_id: receipt_id.into(),
        };
        if !valid_id(&request.request_id) || !valid_id(&request.receipt_id) {
            return Err(PrintError::new(PrintErrorCode::InvalidRequest));
        }
        Ok(request)
    }

    /// Decode the closed schema. Unknown device, byte, destination, and path fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` for malformed JSON or invalid fields.
    pub fn decode(bytes: &[u8]) -> Result<Self, PrintError> {
        let request: Self = serde_json::from_slice(bytes)
            .map_err(|_| PrintError::new(PrintErrorCode::InvalidRequest))?;
        Self::new(request.request_id, request.receipt_id)
    }
}

/// Host-rendered preview projection of an immutable receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintPreview {
    receipt: Receipt,
}

impl PrintPreview {
    /// Verified merchant shown by the host.
    #[must_use]
    pub fn merchant(&self) -> &str {
        self.receipt.merchant()
    }

    /// Ordered structured lines shown by the host.
    #[must_use]
    pub fn lines(&self) -> &[ReceiptLine] {
        self.receipt.lines()
    }

    /// Exact total shown by the host.
    #[must_use]
    pub const fn total(&self) -> &crate::Money {
        self.receipt.total()
    }
}

/// One accepted in-memory preview job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintJob {
    id: String,
    receipt_id: String,
    preview: PrintPreview,
}

impl PrintJob {
    /// Host job identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Approved receipt identity.
    #[must_use]
    pub fn receipt_id(&self) -> &str {
        &self.receipt_id
    }

    /// Host-owned preview model.
    #[must_use]
    pub const fn preview(&self) -> &PrintPreview {
        &self.preview
    }
}

/// In-memory simulator with no device or network channel.
#[derive(Default)]
pub struct PrinterSimulator {
    receipts: HashMap<String, Receipt>,
    jobs: HashMap<String, PrintJob>,
    next_job: u64,
}

impl PrinterSimulator {
    /// Create an empty simulator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
            jobs: HashMap::new(),
            next_job: 1,
        }
    }

    /// Register one host-created approved receipt.
    ///
    /// # Errors
    ///
    /// Rejects a duplicate receipt identity.
    pub fn register(&mut self, receipt: Receipt) -> Result<(), PrintError> {
        if self.receipts.contains_key(receipt.id()) {
            return Err(PrintError::new(PrintErrorCode::InvalidRequest));
        }
        self.receipts.insert(receipt.id().to_owned(), receipt);
        Ok(())
    }

    /// Create exactly one host preview for an owned, approved receipt request.
    ///
    /// # Errors
    ///
    /// Rejects duplicate requests, missing receipts, or cross-principal access.
    pub fn preview(
        &mut self,
        owner: &PluginPrincipal,
        request: PrintPreviewRequest,
    ) -> Result<PrintJob, PrintError> {
        if self.jobs.contains_key(&request.request_id) {
            return Err(PrintError::new(PrintErrorCode::DuplicateRequest));
        }
        let receipt = self
            .receipts
            .get(&request.receipt_id)
            .ok_or_else(|| PrintError::new(PrintErrorCode::ReceiptNotFound))?;
        if receipt.owner() != owner {
            return Err(PrintError::new(PrintErrorCode::OwnerMismatch));
        }
        let job = PrintJob {
            id: format!("print-job-{:08}", self.next_job),
            receipt_id: receipt.id().to_owned(),
            preview: PrintPreview {
                receipt: receipt.clone(),
            },
        };
        self.next_job = self.next_job.saturating_add(1);
        self.jobs.insert(request.request_id, job.clone());
        Ok(job)
    }

    /// Number of accepted jobs.
    #[must_use]
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    /// Simulator invariant: no raw device channel exists.
    #[must_use]
    pub const fn device_writes(&self) -> usize {
        0
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
