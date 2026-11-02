//! Native Runtime guest host and platform feasibility probe.

use studio_runtime::gpui_https_client;

use std::sync::Arc;

use gpui_kit::assets::Assets;
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::platform::application;
use gpui_kit::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use studio_app::{
    cli::LaunchRequest, foundation::FoundationGallery, host::NativeDisplayAvailability,
    plugin_surface::PluginSurface,
};
use studio_package::TrustStore;

fn run(
    application: Application,
    plugin_surface: Option<PluginSurface>,
    https_client: Arc<gpui_https_client::GpuiHttpsClient>,
) {
    application.run(move |cx: &mut App| {
        let _ = https_client.install(cx.http_client());
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        let has_plugin_surface = plugin_surface.is_some();
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
                if has_plugin_surface {
                    let shell = shell.downgrade();
                    cx.spawn(async move |cx| {
                        use std::time::Duration;
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(100))
                                .await;
                            let alive = shell.update(&mut cx.clone(), |shell, cx| {
                                match shell.pump_host_actions(cx) {
                                    Ok(changed) => {
                                        if changed {
                                            cx.notify();
                                        }
                                        true
                                    }
                                    Err(_) => false,
                                }
                            });
                            if !alive.is_ok_and(|poll_ok| poll_ok) {
                                eprintln!("provider action response could not be applied");
                                break;
                            }
                        }
                    })
                    .detach();
                }
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .expect("Studio could not create its native window");
        cx.activate(true);
    });
}

fn main() {
    let display = NativeDisplayAvailability::from_environment();
    if display == NativeDisplayAvailability::Unavailable {
        eprintln!("Studio requires a graphical session (Wayland or X11 on Linux).");
        std::process::exit(2);
    }

    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.len() == 1 {
        eprintln!("usage: studio-app --bundle <absolute-path>");
        std::process::exit(2);
    }
    let (plugin_surface, https_client) = {
        let request = match LaunchRequest::parse_runtime_from(arguments) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
        let trust_store = match TrustStore::load_from_environment() {
            Ok(store) => store,
            Err(error) => {
                eprintln!(
                    "{}",
                    studio_app::host::LaunchError::TrustConfigurationInvalid(error)
                );
                std::process::exit(2);
            }
        };
        let https_client = Arc::new(gpui_https_client::GpuiHttpsClient::default());
        let host = match studio_runtime::bootstrap::host_from_environment(
            trust_store,
            https_client.clone(),
        ) {
            Ok(host) => host,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
        match host.prepare(request) {
            Ok(surface) => {
                if let Some(warning) = surface.warning() {
                    eprintln!("{warning}");
                }
                (Some(surface), https_client)
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
        https_client,
    );
}
