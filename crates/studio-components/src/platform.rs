//! Native display preflight shared by the Runtime and Designer.

use std::ffi::OsStr;

/// Whether the current platform has a possible graphical session.
///
/// This is a preflight only: GPUI opens the display and reports connection errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeDisplayAvailability {
    /// A supported native display may be opened.
    Available,
    /// Linux has neither a Wayland endpoint nor an X11 display.
    Unavailable,
}

impl NativeDisplayAvailability {
    /// Detect Linux display endpoints; other platforms use GPUI's native backend.
    #[must_use]
    pub fn from_environment() -> Self {
        if cfg!(target_os = "linux") {
            Self::from_linux_endpoints(
                std::env::var_os("WAYLAND_DISPLAY").as_deref(),
                std::env::var_os("WAYLAND_SOCKET").as_deref(),
                std::env::var_os("DISPLAY").as_deref(),
            )
        } else {
            Self::Available
        }
    }

    fn from_linux_endpoints(
        wayland_display: Option<&OsStr>,
        wayland_socket: Option<&OsStr>,
        x11_display: Option<&OsStr>,
    ) -> Self {
        if [wayland_display, wayland_socket, x11_display]
            .into_iter()
            .any(|value| value.is_some_and(|value| !value.is_empty()))
        {
            Self::Available
        } else {
            Self::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NativeDisplayAvailability::{self, Available, Unavailable};
    use std::ffi::OsStr;

    #[test]
    fn accepts_wayland_display_or_inherited_socket() {
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(
                Some(OsStr::new("wayland-0")),
                None,
                None
            ),
            Available
        );
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(None, Some(OsStr::new("4")), None),
            Available
        );
    }

    #[test]
    fn accepts_x11_without_wayland() {
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(None, None, Some(OsStr::new(":0"))),
            Available
        );
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(
                Some(OsStr::new("")),
                None,
                Some(OsStr::new(":1"))
            ),
            Available
        );
    }

    #[test]
    fn rejects_missing_or_empty_linux_endpoints() {
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(None, None, None),
            Unavailable
        );
        assert_eq!(
            NativeDisplayAvailability::from_linux_endpoints(
                Some(OsStr::new("")),
                Some(OsStr::new("")),
                Some(OsStr::new(""))
            ),
            Unavailable
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn native_platform_does_not_require_linux_display_variables() {
        assert_eq!(NativeDisplayAvailability::from_environment(), Available);
    }
}
