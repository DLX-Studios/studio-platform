//! Closed command-line selection for production and development bundles.

use std::{ffi::OsString, path::PathBuf};

use crate::host::LaunchError;

/// Explicit bundle trust mode selected at startup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchMode {
    /// Signature and provisioned publisher trust are mandatory.
    Production,
    /// This one explicitly selected local bundle may be unsigned.
    Development,
}

/// Parsed startup selection; validation of the target file remains host-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchRequest {
    mode: LaunchMode,
    path: PathBuf,
}

impl LaunchRequest {
    /// Parse exactly `--bundle <path>` or `--dev <path>`.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError::ArgumentsInvalid`] for missing, conflicting, or extra arguments.
    pub fn parse_from<I, T>(arguments: I) -> Result<Self, LaunchError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program = arguments.next();
        let selector = arguments.next().ok_or(LaunchError::ArgumentsInvalid)?;
        let path = arguments.next().ok_or(LaunchError::ArgumentsInvalid)?;
        if arguments.next().is_some() {
            return Err(LaunchError::ArgumentsInvalid);
        }
        let mode = match selector.to_str() {
            Some("--bundle") => LaunchMode::Production,
            Some("--dev") => LaunchMode::Development,
            _ => return Err(LaunchError::ArgumentsInvalid),
        };
        if path.is_empty() {
            return Err(LaunchError::ArgumentsInvalid);
        }
        Ok(Self {
            mode,
            path: PathBuf::from(path),
        })
    }

    /// Selected trust mode.
    #[must_use]
    pub const fn mode(&self) -> LaunchMode {
        self.mode
    }

    /// Explicit local bundle path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) fn into_parts(self) -> (LaunchMode, PathBuf) {
        (self.mode, self.path)
    }
}
