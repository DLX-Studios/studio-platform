//! Host-owned print-preview surface model.

use studio_actions::{PrintJob, PrintPreview};

/// Read-only model used by the native preview overlay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintPreviewSurface {
    job: PrintJob,
}

impl PrintPreviewSurface {
    /// Construct the trusted surface from one accepted simulator job.
    #[must_use]
    pub const fn new(job: PrintJob) -> Self {
        Self { job }
    }

    /// Host-created job identity.
    #[must_use]
    pub fn job_id(&self) -> &str {
        self.job.id()
    }

    /// Structured preview rendered by Studio.
    #[must_use]
    pub const fn preview(&self) -> &PrintPreview {
        self.job.preview()
    }
}
