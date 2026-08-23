//! The compiler boundary for Studio Script source files.
//!
//! This crate deliberately owns the Studio-facing API rather than exposing a
//! particular Svelte compiler directly. The initial implementation prepares a
//! source file and separates its script block from its component markup. A
//! compiler adapter can later turn the prepared source into JavaScript or an
//! AssemblyScript-compatible production artifact without changing callers.

use std::path::{Path, PathBuf};

/// Errors reported while preparing a Studio Script source file.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The source contains more than one supported script block.
    #[error("{filename} contains more than one <script> block")]
    MultipleScriptBlocks {
        /// The source file containing the duplicate blocks.
        filename: PathBuf,
    },

    /// The closing script tag is missing.
    #[error("{filename} contains an unterminated <script> block")]
    UnterminatedScriptBlock {
        /// The source file containing the unterminated block.
        filename: PathBuf,
    },

    /// A closing script tag appears without an opening script tag.
    #[error("{filename} contains a closing </script> without an opening <script>")]
    UnexpectedScriptClose {
        /// The source file containing the unexpected closing tag.
        filename: PathBuf,
    },
}

/// The intended output backend for a future Studio Script transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Development or web output executed by a JavaScript runtime.
    JavaScript,
    /// Production output that will be lowered to `AssemblyScript` and Wasm.
    AssemblyScript,
}

/// The source blocks extracted from a `.studio` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlocks {
    /// The contents of the optional `<script>` block.
    pub script: Option<String>,
    /// Markup outside the `<script>` block.
    pub markup: String,
}

/// A source file prepared for a compiler backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSource {
    /// The source filename used for diagnostics and module identity.
    pub filename: PathBuf,
    /// The selected output target.
    pub target: Target,
    /// The separated source blocks.
    pub blocks: SourceBlocks,
}

/// Prepare a Studio Script source file for a compiler backend.
///
/// # Errors
///
/// Returns [`Error`] when the source contains malformed, duplicate, or
/// unexpected script tags.
pub fn prepare(
    path: impl AsRef<Path>,
    source: &str,
    target: Target,
) -> Result<PreparedSource, Error> {
    let filename = path.as_ref().to_path_buf();
    let blocks = split_source(&filename, source)?;

    Ok(PreparedSource {
        filename,
        target,
        blocks,
    })
}

fn split_source(filename: &Path, source: &str) -> Result<SourceBlocks, Error> {
    let Some(open_start) = source.find("<script") else {
        if source.contains("</script>") {
            return Err(Error::UnexpectedScriptClose {
                filename: filename.to_path_buf(),
            });
        }

        return Ok(SourceBlocks {
            script: None,
            markup: source.to_owned(),
        });
    };

    let open_end = source[open_start..]
        .find('>')
        .map(|offset| open_start + offset)
        .ok_or_else(|| Error::UnterminatedScriptBlock {
            filename: filename.to_path_buf(),
        })?;
    let content_start = open_end + 1;
    let close_relative = source[content_start..].find("</script>").ok_or_else(|| {
        Error::UnterminatedScriptBlock {
            filename: filename.to_path_buf(),
        }
    })?;
    let close_start = content_start + close_relative;
    let after_close = close_start + "</script>".len();

    if source[after_close..].contains("<script") {
        return Err(Error::MultipleScriptBlocks {
            filename: filename.to_path_buf(),
        });
    }

    let mut markup = String::with_capacity(source.len() - (open_end - open_start + 1));
    markup.push_str(&source[..open_start]);
    markup.push_str(&source[after_close..]);

    Ok(SourceBlocks {
        script: Some(source[content_start..close_start].to_owned()),
        markup,
    })
}

#[cfg(test)]
mod tests {
    use super::{Error, Target, prepare};
    use std::path::Path;

    #[test]
    fn splits_script_and_markup() {
        let prepared = prepare(
            "Counter.studio",
            "<script lang=\"ts\">let count = $state(0)</script><button>{count}</button>",
            Target::JavaScript,
        )
        .expect("source should split");

        assert_eq!(prepared.filename, Path::new("Counter.studio"));
        assert_eq!(
            prepared.blocks.script.as_deref(),
            Some("let count = $state(0)")
        );
        assert_eq!(prepared.blocks.markup, "<button>{count}</button>");
    }

    #[test]
    fn supports_markup_without_script() {
        let prepared = prepare("Card.studio", "<Card />", Target::AssemblyScript)
            .expect("source should split");

        assert_eq!(prepared.blocks.script, None);
        assert_eq!(prepared.blocks.markup, "<Card />");
    }

    #[test]
    fn rejects_multiple_script_blocks() {
        let error = prepare(
            "Broken.studio",
            "<script>one</script><Card /><script>two</script>",
            Target::JavaScript,
        )
        .expect_err("multiple scripts should fail");

        assert_eq!(
            error,
            Error::MultipleScriptBlocks {
                filename: "Broken.studio".into()
            }
        );
    }
}
