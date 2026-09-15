//! Validated retained UI trees and atomic mutation transactions.

mod error;
mod mount;
mod node;
mod patch;
mod registry;
mod transaction;

pub use error::UiDiagnostic;
pub use mount::{MountError, MountErrorCode};
pub use node::{InstanceId, RetainedNode};
pub use patch::{CommittedChange, PatchCommit, PatchError, PatchErrorCode};
pub use registry::{PatchMetrics, RegistrySnapshot, UiRegistry};
