//! Host-resolved property transition policy.

use std::time::Duration;

use studio_navigation::MotionPreference;

/// A bounded property transition resolved under the host motion preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyTransition {
    duration: Duration,
}

impl PropertyTransition {
    /// Clamp a requested duration to one second, or zero it for reduced motion.
    #[must_use]
    pub fn resolve(requested: Duration, preference: MotionPreference) -> Self {
        let duration = match preference {
            MotionPreference::Reduced => Duration::ZERO,
            MotionPreference::Standard => requested.min(Duration::from_secs(1)),
        };
        Self { duration }
    }

    /// Return the host-approved duration.
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.duration
    }
}
