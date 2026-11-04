//! Host-owned route matching, bounded stacks, guards, and transitions.

mod error;
mod guard;
pub mod project;
mod route;
mod stack;
pub mod timers;
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
