//! Wayland-only Runtime guest host and platform feasibility probe.

use std::ffi::OsStr;

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode};
use gpui_kit_assets::Assets;
use gpui_platform::application;
use studio_app::{
    cli::LaunchRequest,
    foundation::FoundationGallery,
    host::{HostConfig, StudioHost, WaylandAvailability},
    plugin_surface::PluginSurface,
};
use studio_package::TrustStore;

fn has_wayland_endpoint(display: Option<&OsStr>, socket: Option<&OsStr>) -> bool {
    display.is_some_and(|value| !value.is_empty()) || socket.is_some_and(|value| !value.is_empty())
}

fn run(
    application: Application,
    plugin_surface: Option<PluginSurface>,
    reload_inbox: Option<std::sync::Arc<studio_app::reload::ReloadInbox>>,
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
                    if let Some(surface) = plugin_surface {
                        FoundationGallery::with_plugin_surface(reduced_motion, surface, window, cx)
                    } else {
                        FoundationGallery::new(reduced_motion, window, cx)
                    }
                });
                // Drain prepared reload swaps without recreating the window:
                // the daemon server thread validates and instantiates, and the
                // frame loop applies each accepted replacement here.
                if let Some(inbox) = reload_inbox {
                    let shell = shell.downgrade();
                    cx.spawn(async move |cx| {
                        use std::time::Duration;
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(250))
                                .await;
                            let swaps = inbox.drain();
                            if swaps.is_empty() {
                                continue;
                            }
                            let applied = shell.update(&mut cx.clone(), |shell, cx| {
                                for swap in swaps {
                                    shell.apply_reload_swap(swap, cx);
                                }
                            });
                            if applied.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                }
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

    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.len() == 1 {
        eprintln!("usage: studio-app (--bundle <absolute-path> | --dev <local-path>)");
        std::process::exit(2);
    }
    let (plugin_surface, reload_inbox) = {
        let request = match LaunchRequest::parse_from(arguments) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
        let trust_store = match request.mode() {
            studio_app::cli::LaunchMode::Production => match TrustStore::load_from_environment() {
                Ok(store) => store,
                Err(error) => {
                    eprintln!(
                        "{}",
                        studio_app::host::LaunchError::TrustConfigurationInvalid(error)
                    );
                    std::process::exit(2);
                }
            },
            studio_app::cli::LaunchMode::Development => TrustStore::default(),
        };
        let socket = request.reload_socket().map(std::path::Path::to_path_buf);
        let host = StudioHost::new(HostConfig::new(trust_store), wayland);
        match host.prepare(request) {
            Ok(surface) => {
                if let Some(warning) = surface.warning() {
                    eprintln!("{warning}");
                }
                let inbox = socket
                    .as_deref()
                    .map(|socket| start_reload_server(host, socket));
                (Some(surface), inbox)
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    };
    run(
        application().with_assets(Assets),
        plugin_surface,
        reload_inbox,
    );
}

/// Never-signalled shutdown flag for the reload server thread: process exit
/// ends the daemon loop, and the next session treats a leftover socket file
/// as stale (refused-as-stale).
static RELOAD_CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Start the reload server thread when `--reload-socket` was requested.
/// Returns the frame-loop inbox; the window drains it per poll cycle.
fn start_reload_server(
    host: StudioHost,
    socket: &std::path::Path,
) -> std::sync::Arc<studio_app::reload::ReloadInbox> {
    use studio_app::reload::{ReloadInbox, ReloadServer, SharedRouteTracker};

    let inbox = std::sync::Arc::new(ReloadInbox::new());
    let tracker = std::sync::Arc::new(SharedRouteTracker::new());
    let listener = match studio_host::reload::bind_endpoint(socket) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "reload channel unavailable at {}: {error}",
                socket.display()
            );
            return inbox;
        }
    };
    let thread_inbox = std::sync::Arc::clone(&inbox);
    std::thread::Builder::new()
        .name("studio-reload".to_owned())
        .spawn(move || {
            let mut server = ReloadServer::new(host, tracker, thread_inbox);
            server.serve(listener, &RELOAD_CANCELLED);
        })
        .expect("reload server thread spawns");
    inbox
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
