//! Wayland-only native shell and platform feasibility probe.

use std::ffi::OsStr;

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode};
use gpui_component_assets::Assets;
use gpui_platform::application;
use studio_app::{
    bootstrap::{NativeProductBootstrap, NativeProductShell},
    cli::LaunchRequest,
    host::{HostConfig, StudioHost, WaylandAvailability},
    plugin_surface::PluginSurface,
};
use studio_package::TrustStore;

fn has_wayland_endpoint(display: Option<&OsStr>, socket: Option<&OsStr>) -> bool {
    display.is_some_and(|value| !value.is_empty()) || socket.is_some_and(|value| !value.is_empty())
}

fn run(
    application: Application,
    bootstrap: NativeProductBootstrap,
    plugin_surface: Option<PluginSurface>,
) {
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            move |window, cx| {
                let reduced_motion = cx.reduce_motion();
                let shell = cx.new(|cx| {
                    NativeProductShell::new(bootstrap, plugin_surface, reduced_motion, window, cx)
                });
                cx.new(|cx| Root::new(shell, window, cx).bordered(false))
            },
        )
        .expect("Studio could not create its Wayland window");
        cx.activate(true);
    });
}

fn main() {
    let wayland = if has_wayland_endpoint(
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("WAYLAND_SOCKET").as_deref(),
    ) {
        WaylandAvailability::Available
    } else {
        eprintln!("Studio requires a native Wayland session; X11 and XWayland are not supported.");
        std::process::exit(2);
    };

    let bootstrap =
        match NativeProductBootstrap::open(NativeProductBootstrap::default_data_directory()) {
            Ok(bootstrap) => bootstrap,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let plugin_surface = if arguments.len() == 1 {
        None
    } else {
        let result = LaunchRequest::parse_from(arguments).and_then(|request| {
            let trust_store = match request.mode() {
                studio_app::cli::LaunchMode::Production => TrustStore::load_from_environment()
                    .map_err(studio_app::host::LaunchError::TrustConfigurationInvalid)?,
                studio_app::cli::LaunchMode::Development => TrustStore::default(),
            };
            StudioHost::new(HostConfig::new(trust_store), wayland).prepare(request)
        });
        match result {
            Ok(surface) => {
                if let Some(warning) = surface.warning() {
                    eprintln!("{warning}");
                }
                Some(surface)
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    };
    run(application().with_assets(Assets), bootstrap, plugin_surface);
}

#[cfg(test)]
mod tests {
    use super::has_wayland_endpoint;
    use std::ffi::OsStr;

    #[test]
    fn accepts_either_wayland_endpoint_variable() {
        assert!(has_wayland_endpoint(Some(OsStr::new("wayland-0")), None));
        assert!(has_wayland_endpoint(None, Some(OsStr::new("4"))));
    }

    #[test]
    fn rejects_missing_or_empty_wayland_endpoint_variables() {
        assert!(!has_wayland_endpoint(None, None));
        assert!(!has_wayland_endpoint(Some(OsStr::new("")), None));
    }
}
