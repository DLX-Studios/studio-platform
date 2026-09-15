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
    reload_socket: Option<PathBuf>,
}

impl LaunchRequest {
    /// Parse exactly `--bundle <path>` or `--dev <path>`, each optionally
    /// followed by `--reload-socket <path>` for the dev reload channel.
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
        let mode = match selector.to_str() {
            Some("--bundle") => LaunchMode::Production,
            Some("--dev") => LaunchMode::Development,
            _ => return Err(LaunchError::ArgumentsInvalid),
        };
        if path.is_empty() {
            return Err(LaunchError::ArgumentsInvalid);
        }
        let mut reload_socket = None;
        if let Some(next) = arguments.next() {
            if next.to_str() != Some("--reload-socket") {
                return Err(LaunchError::ArgumentsInvalid);
            }
            let socket = arguments.next().ok_or(LaunchError::ArgumentsInvalid)?;
            if socket.is_empty() {
                return Err(LaunchError::ArgumentsInvalid);
            }
            reload_socket = Some(PathBuf::from(socket));
        }
        if arguments.next().is_some() {
            return Err(LaunchError::ArgumentsInvalid);
        }
        Ok(Self {
            mode,
            path: PathBuf::from(path),
            reload_socket,
        })
    }

    /// Selected trust mode.
    #[must_use]
    pub const fn mode(&self) -> LaunchMode {
        self.mode
    }

    /// Construct a request directly (the reload server builds these; the CLI
    /// arrows in `parse_from` remain the only command-line shape).
    #[must_use]
    pub fn new(mode: LaunchMode, path: PathBuf) -> Self {
        Self {
            mode,
            path,
            reload_socket: None,
        }
    }

    /// Reload channel socket requested at startup, if any.
    #[must_use]
    pub fn reload_socket(&self) -> Option<&std::path::Path> {
        self.reload_socket.as_deref()
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
