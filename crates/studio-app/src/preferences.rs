//! Host-owned accessibility preferences.

use studio_navigation::MotionPreference;

use crate::settings::GlobalSettings;

/// Effective preferences read from the native host environment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostPreferences {
    reduced_motion: bool,
}

impl HostPreferences {
    /// Capture an effective reduced-motion setting.
    #[must_use]
    pub const fn new(reduced_motion: bool) -> Self {
        Self { reduced_motion }
    }

    /// Resolve the setting for navigation and property transitions.
    #[must_use]
    pub const fn motion(self) -> MotionPreference {
        if self.reduced_motion {
            MotionPreference::Reduced
        } else {
            MotionPreference::Standard
        }
    }

    /// Resolve host navigation preferences from persisted global settings.
    #[must_use]
    pub const fn from_global(settings: &GlobalSettings) -> Self {
        Self::new(settings.reduced_motion)
    }
}
