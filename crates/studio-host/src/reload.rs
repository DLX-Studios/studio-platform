//! CLI-to-runtime reload protocol: versioned messages, session revisions,
//! and the atomic swap state machine.
//!
//! Wire types live in `studio_protocol::reload` so the toolchain can speak the
//! protocol without the host's storage dependencies. This module adds the
//! server-side planner: session revisions and swap decisions.

pub use studio_protocol::reload::{
    DecodedOutcome, RELOAD_PROTOCOL, RELOAD_SOCKET_NAME, ReloadOutcome, ReloadRequest, ReloadStage,
    RestartOutcome, bind_endpoint, connect_endpoint, decode_outcome, encode_request,
    encode_restart, read_line, send_line, socket_path,
};

use std::collections::VecDeque;

/// Monotonic session revision owned by the runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionRevision(u64);

impl SessionRevision {
    /// First revision after the initial mount.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Current revision value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advance exactly once per accepted reload or restart.
    pub fn advance(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}

/// Planned swap decision: what the runtime prepared and what happens next.
#[derive(Clone, Debug, PartialEq)]
pub enum SwapPlan {
    /// Preparation succeeded; committing swaps and disposes the old instance.
    Commit {
        /// Prepared bundle path.
        bundle: String,
    },
    /// Preparation failed; the live surface stays untouched.
    Reject {
        /// Failing stage.
        stage: ReloadStage,
        /// Stable diagnostic code.
        code: String,
        /// Safe message.
        message: String,
    },
}

/// Pure swap planner: validate the transition without touching live state.
///
/// The caller performs validation/instantiation and reports the result here;
/// the planner decides commit vs rollback and tracks revision accounting.
/// Disposal of the old instance happens only after commit.
pub struct SwapSession {
    revision: SessionRevision,
    pending: VecDeque<ReloadRequest>,
    trailing: Option<ReloadRequest>,
}

impl SwapSession {
    /// Open a session at the initial mount revision.
    #[must_use]
    pub fn new() -> Self {
        Self {
            revision: SessionRevision::initial(),
            pending: VecDeque::new(),
            trailing: None,
        }
    }

    /// Current session revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Offer one reload request. While a swap executes, the request becomes
    /// the single trailing reload (latest wins); otherwise it queues.
    pub fn offer(&mut self, request: ReloadRequest, swapping: bool) {
        if swapping {
            self.trailing = Some(request);
        } else {
            self.pending.push_back(request);
        }
    }

    /// Take the next request to execute, if any.
    pub fn take_next(&mut self) -> Option<ReloadRequest> {
        self.pending.pop_front()
    }

    /// After a swap completes, promote the trailing request (if any) to the
    /// single next reload with the latest bundle.
    pub fn promote_trailing(&mut self) {
        if let Some(request) = self.trailing.take() {
            self.pending.push_back(request);
        }
    }

    /// Record a committed swap: revision advances exactly once.
    pub fn committed(&mut self) -> u64 {
        self.revision.advance()
    }

    /// Plan the outcome of one preparation result.
    #[must_use]
    pub fn plan(
        &self,
        request: &ReloadRequest,
        prepared: Result<String, (ReloadStage, String, String)>,
    ) -> (SwapPlan, ReloadOutcome) {
        match prepared {
            Ok(bundle) => (
                SwapPlan::Commit {
                    bundle: bundle.clone(),
                },
                ReloadOutcome::Reloaded {
                    request_id: request.request_id.clone(),
                    revision: self.revision.get() + 1,
                },
            ),
            Err((stage, code, message)) => (
                SwapPlan::Reject {
                    stage,
                    code: code.clone(),
                    message: message.clone(),
                },
                ReloadOutcome::Rejected {
                    request_id: request.request_id.clone(),
                    stage,
                    code,
                    message,
                },
            ),
        }
    }
}

impl Default for SwapSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str, bundle: &str) -> ReloadRequest {
        ReloadRequest {
            request_id: id.to_owned(),
            bundle: bundle.to_owned(),
            base_revision: 1,
            declared_routes: Vec::new(),
        }
    }

    #[test]
    fn supersede_keeps_exactly_one_trailing_reload() {
        let mut session = SwapSession::new();
        session.offer(request("r-1", "/tmp/a.studio"), false);
        session.offer(request("r-2", "/tmp/b.studio"), true);
        session.offer(request("r-3", "/tmp/c.studio"), true);
        assert_eq!(session.take_next().unwrap().request_id, "r-1");
        assert!(session.take_next().is_none());
        session.promote_trailing();
        assert_eq!(session.take_next().unwrap().bundle, "/tmp/c.studio");
        assert!(session.take_next().is_none());
    }

    #[test]
    fn revision_advances_exactly_once_per_commit() {
        let mut session = SwapSession::new();
        assert_eq!(session.revision(), 1);
        let request = request("r-1", "/tmp/a.studio");
        let (plan, outcome) = session.plan(&request, Ok("/tmp/a.studio".to_owned()));
        assert!(matches!(plan, SwapPlan::Commit { .. }));
        assert!(matches!(
            outcome,
            ReloadOutcome::Reloaded { revision: 2, .. }
        ));
        assert_eq!(session.committed(), 2);
        assert_eq!(session.revision(), 2);
    }
}
