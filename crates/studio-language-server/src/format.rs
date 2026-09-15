//! Deterministic `.studio` formatting through the vendored formatter.
//!
//! Studio sources are Svelte-shaped, so the adapter presents them to the
//! Svelte pipeline under a `.svelte` filename: the content decides the
//! formatting, never the author's extension. Formatting is pure text in,
//! text out — no network, no Node.js, no side effects.

use std::path::{Path, PathBuf};

/// Format one `.studio` source deterministically.
///
/// Returns the formatted text, or a safe message when the upstream formatter
/// rejects the input (the caller degrades to a diagnostic and returns the
/// source unchanged).
///
/// # Errors
///
/// Returns a message when resolution or formatting fails.
pub fn format_studio(source: &str, real_path: &Path) -> Result<String, String> {
    let session = rsvelte_fmt::FormatSession::resolve(real_path)
        .map_err(|error| format!("formatter unavailable: {error}"))?;
    // The dispatch key is the extension; present Svelte-shaped content as
    // Svelte so it stays in the in-process pipeline and never reaches the
    // external oxfmt fallback.
    let dispatch = real_path.with_extension("svelte");
    session
        .format(source, &dispatch)
        .map_err(|error| format!("formatter rejected the source: {error}"))
}

/// Check idempotence: formatting twice must equal formatting once.
#[must_use]
pub fn is_stable(source: &str, real_path: &Path) -> bool {
    match format_studio(source, real_path) {
        Ok(first) => format_studio(&first, real_path).is_ok_and(|second| second == first),
        Err(_) => false,
    }
}

/// A virtual path for untitled or in-memory buffers.
#[must_use]
pub fn virtual_svelte_path() -> PathBuf {
    std::env::temp_dir().join("studio-language-server/untitled.svelte")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_trivial_sources_format() {
        let path = virtual_svelte_path();
        let formatted = format_studio("<Card id=\"a\" />\n", &path).unwrap();
        assert!(formatted.contains("<Card"));
    }
}
