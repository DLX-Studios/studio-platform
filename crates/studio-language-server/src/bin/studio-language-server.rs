#![allow(missing_docs)]

use std::env;

use studio_language_server::{LanguageServer, Workspace};

fn main() -> std::io::Result<()> {
    let workspace = match env::args_os().nth(1) {
        Some(root) => Workspace::from_root(root)?,
        None => Workspace::new(),
    };
    LanguageServer::new(workspace).serve(std::io::stdin().lock(), std::io::stdout().lock())
}
