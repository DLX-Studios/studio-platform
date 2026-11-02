//! Native Studio Designer application.

use gpui_kit::assets::Assets;
use gpui_kit::component::{Root, Theme, ThemeMode, TitleBar};
use gpui_kit::platform::application;
use gpui_kit::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use studio_designer::bootstrap::{NativeProductBootstrap, NativeProductShell};

fn run(application: Application, bootstrap: NativeProductBootstrap) {
    application.run(move |cx: &mut App| {
        gpui_kit::init(cx);
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
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .expect("Studio Designer could not create its native window");
        cx.activate(true);
    });
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.first().and_then(|argument| argument.to_str()) == Some("agent-server") {
        let mut project_root = None;
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].to_str() {
                Some("--project-root") => {
                    index += 1;
                    if index >= arguments.len() {
                        eprintln!("usage: studio-designer agent-server [--project-root PATH]");
                        std::process::exit(2);
                    }
                    project_root = Some(std::path::PathBuf::from(&arguments[index]));
                }
                Some("--help" | "-h") => {
                    println!("usage: studio-designer agent-server [--project-root PATH]");
                    return;
                }
                _ => {
                    eprintln!("usage: studio-designer agent-server [--project-root PATH]");
                    std::process::exit(2);
                }
            }
            index += 1;
        }
        let root = match project_root {
            Some(root) => root,
            None => match std::env::current_dir() {
                Ok(root) => root,
                Err(error) => {
                    eprintln!("Studio Designer agent server: {error}");
                    std::process::exit(2);
                }
            },
        };
        std::process::exit(studio_designer::agent_rpc::run_stdio(&root));
    }
    if !arguments.is_empty() {
        eprintln!("usage: studio-designer [agent-server [--project-root PATH]]");
        std::process::exit(2);
    }

    if !studio_designer::bootstrap::native_display_available() {
        eprintln!("Studio Designer requires a graphical session (Wayland or X11 on Linux).");
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
