//! Wayland-only Studio Designer application.

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use gpui_component_assets::Assets;
use gpui_platform::application;
use studio_designer::bootstrap::{NativeProductBootstrap, NativeProductShell};

fn run(application: Application, bootstrap: NativeProductBootstrap) {
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitleBar::title_bar_options()),
                ..WindowOptions::default()
            },
            move |window, cx| {
                let reduced_motion = cx.reduce_motion();
                let shell = cx.new(|_cx| NativeProductShell::new(bootstrap, reduced_motion));
                cx.new(|cx| Root::new(shell, window, cx).bordered(false))
            },
        )
        .expect("Studio Designer could not create its Wayland window");
        cx.activate(true);
    });
}

fn main() {
    if !studio_designer::bootstrap::wayland_available() {
        eprintln!(
            "Studio Designer requires a native Wayland session; X11 and XWayland are not supported."
        );
        std::process::exit(2);
    }

    let bootstrap =
        match NativeProductBootstrap::open(NativeProductBootstrap::default_data_directory()) {
            Ok(bootstrap) => bootstrap,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        };
    run(application().with_assets(Assets), bootstrap);
}
