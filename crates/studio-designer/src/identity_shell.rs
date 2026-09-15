//! Small app-facing state seam for the welcome and identity chooser.
//!
//! Persistence and authentication remain in [`studio_host::IdentityService`].
//! This model only translates its owned snapshot into deterministic native
//! shell routes, so a GPUI view cannot accidentally become the security
//! authority.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]

use studio_host::{
    IdentitySession, IdentitySnapshot, IdentityState, IdentitySummary, SessionSummary,
};

/// Native-shell route for the identity journey.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityShellRoute {
    /// First-launch product welcome.
    Welcome,
    /// Account chooser and remembered-session list.
    Chooser,
    /// Local identity creation form.
    CreateIdentity,
    /// Password sign-in form for one available identity.
    SignIn { identity_id: String },
    /// Password gate for one identity locked by a failed attempt.
    Unlock { identity_id: String },
    /// Project entry after host authentication.
    ProjectEntry {
        identity_id: String,
        session_id: String,
    },
}

/// Testable state model consumed by the native identity shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityShellState {
    route: IdentityShellRoute,
    snapshot: IdentitySnapshot,
}

impl IdentityShellState {
    /// Build initial startup state from the host-owned identity snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: IdentitySnapshot) -> Self {
        let route = if snapshot.welcome_dismissed {
            IdentityShellRoute::Chooser
        } else {
            IdentityShellRoute::Welcome
        };
        Self { route, snapshot }
    }

    /// Current route selected by the shell.
    #[must_use]
    pub const fn route(&self) -> &IdentityShellRoute {
        &self.route
    }

    /// Current immutable host snapshot shown by the shell.
    #[must_use]
    pub const fn snapshot(&self) -> &IdentitySnapshot {
        &self.snapshot
    }

    /// Apply a fresh host snapshot after a persistence or authentication call.
    pub fn refresh(&mut self, snapshot: IdentitySnapshot) {
        self.snapshot = snapshot;
        if matches!(self.route, IdentityShellRoute::Welcome) && self.snapshot.welcome_dismissed {
            self.route = IdentityShellRoute::Chooser;
        }
    }

    /// Dismiss welcome locally after the host has persisted the dismissal.
    pub fn dismiss_welcome(&mut self) {
        self.snapshot.welcome_dismissed = true;
        self.route = IdentityShellRoute::Chooser;
    }

    /// Re-open welcome from Help; the host should persist this choice through
    /// [`studio_host::IdentityService::revisit_welcome`].
    pub fn revisit_welcome(&mut self) {
        self.snapshot.welcome_dismissed = false;
        self.route = IdentityShellRoute::Welcome;
    }

    /// Open the local identity creation form.
    pub fn begin_create_identity(&mut self) {
        self.route = IdentityShellRoute::CreateIdentity;
    }

    /// Select an identity and route to its appropriate password gate.
    ///
    /// Returns `false` when the identity is absent from the host snapshot.
    pub fn choose_identity(&mut self, identity_id: &str) -> bool {
        let Some(identity) = self
            .snapshot
            .identities
            .iter()
            .find(|identity| identity.identity_id == identity_id)
        else {
            return false;
        };
        self.route = match identity.state {
            IdentityState::Available => IdentityShellRoute::SignIn {
                identity_id: identity.identity_id.clone(),
            },
            IdentityState::Locked => IdentityShellRoute::Unlock {
                identity_id: identity.identity_id.clone(),
            },
        };
        true
    }

    /// Enter the project shell only with a session returned by the host.
    ///
    /// Returns `false` if the session's identity is not in this snapshot.
    pub fn enter_project(&mut self, session: &IdentitySession) -> bool {
        if !self
            .snapshot
            .identities
            .iter()
            .any(|identity| identity.identity_id == session.identity_id())
        {
            return false;
        }
        self.route = IdentityShellRoute::ProjectEntry {
            identity_id: session.identity_id().to_owned(),
            session_id: session.session_id().to_owned(),
        };
        true
    }

    /// Identities available to render in the chooser.
    #[must_use]
    pub fn identities(&self) -> &[IdentitySummary] {
        &self.snapshot.identities
    }

    /// Remembered sessions available to render in the chooser.
    #[must_use]
    pub fn sessions(&self) -> &[SessionSummary] {
        &self.snapshot.sessions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_host::{IdentityKind, SessionState};

    fn snapshot(dismissed: bool, state: IdentityState) -> IdentitySnapshot {
        IdentitySnapshot {
            welcome_dismissed: dismissed,
            identities: vec![IdentitySummary {
                identity_id: "local-1".to_owned(),
                kind: IdentityKind::Local,
                display_name: "Alice".to_owned(),
                email: None,
                avatar: None,
                state,
            }],
            sessions: vec![SessionSummary {
                session_id: "session-1".to_owned(),
                identity_id: "local-1".to_owned(),
                remembered: true,
                state: SessionState::Available,
            }],
        }
    }

    #[test]
    fn startup_and_reopen_follow_persisted_welcome_choice() {
        let mut shell =
            IdentityShellState::from_snapshot(snapshot(false, IdentityState::Available));
        assert_eq!(shell.route(), &IdentityShellRoute::Welcome);
        shell.dismiss_welcome();
        assert_eq!(shell.route(), &IdentityShellRoute::Chooser);
        shell.revisit_welcome();
        assert_eq!(shell.route(), &IdentityShellRoute::Welcome);
        shell.refresh(snapshot(true, IdentityState::Available));
        assert_eq!(shell.route(), &IdentityShellRoute::Chooser);
    }

    #[test]
    fn chooser_routes_available_and_locked_identities_to_distinct_gates() {
        let mut shell = IdentityShellState::from_snapshot(snapshot(true, IdentityState::Available));
        assert!(shell.choose_identity("local-1"));
        assert_eq!(
            shell.route(),
            &IdentityShellRoute::SignIn {
                identity_id: "local-1".to_owned()
            }
        );
        shell.refresh(snapshot(true, IdentityState::Locked));
        assert!(shell.choose_identity("local-1"));
        assert_eq!(
            shell.route(),
            &IdentityShellRoute::Unlock {
                identity_id: "local-1".to_owned()
            }
        );
    }
}
