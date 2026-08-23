//! Host-owned route matching, bounded stacks, guards, and transitions.

mod error;
mod guard;
mod route;
mod stack;
mod transition;
mod tree;

pub use error::{StackError, StackErrorCode};
pub use guard::{GuardDecision, GuardResponse, NavigationGuard};
pub use route::{RouteDefinition, RouteError, RouteErrorCode};
pub use stack::{NavigationOperation, NavigationStack, StackOwner};
pub use transition::{
    HostClock, MotionPreference, RouteTransition, TransitionController, TransitionKind,
    TransitionState,
};
pub use tree::{RouteResolution, RouteTree};
