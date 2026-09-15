//! Studio toolchain library: phased build pipeline and the dev watch loop.
//!
//! The `studio` binary is a thin clap front end over these modules so the
//! pipeline and watcher stay unit-testable.

pub mod assets;
pub mod build;
pub mod new;
pub mod routes;
pub mod watch;
