//! Native project commands share the MCP process service and dev JSON-RPC.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui_kit::{
    Context, EventEmitter, FocusHandle, FontWeight, KeyBinding, PathPromptOptions, Role, Window,
    actions,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
    },
    div,
    prelude::*,
};
use serde_json::{Value, json};
use studio_protocol::dev_rpc::{self, DevPhase, DevStatus};

use crate::agent_process::Processes;

actions!(designer_project, [OpenProjectFolder]);

struct ProjectKeyBindings;
impl gpui_kit::Global for ProjectKeyBindings {}

#[derive(Clone)]
pub(crate) enum ProjectControlsEvent {
    OpenFolder(PathBuf),
    PreviewStatus(Option<DevStatus>),
}

#[derive(Clone, Copy)]
enum ProjectCommand {
    Preview,
    Restart,
    Stop,
    Check,
    Build,
    Cancel,
}

impl ProjectCommand {
    fn tool(self) -> &'static str {
        match self {
            Self::Preview => "preview.start",
            Self::Restart => "preview.restart",
            Self::Stop => "preview.stop",
            Self::Check => "studio.check",
            Self::Build => "studio.build",
            Self::Cancel => "studio.task.cancel",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Preview => "Opening preview",
            Self::Restart => "Restarting preview",
            Self::Stop => "Stopping preview",
            Self::Check => "Checking source files",
            Self::Build => "Building project",
            Self::Cancel => "Cancelling task",
        }
    }
}

#[derive(Default, PartialEq)]
struct ProcessSnapshot {
    preview: Value,
    task: Value,
    status: Option<DevStatus>,
    rpc_error: Option<String>,
}

impl ProcessSnapshot {
    fn read(processes: &Arc<Mutex<Processes>>, root: &std::path::Path) -> Self {
        let (preview, task) = {
            let mut processes = processes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let read = |processes: &mut Processes, tool| {
                processes
                    .call(tool, &json!({}), root)
                    .unwrap()
                    .unwrap_or_else(|(_, message)| json!({ "phase": "failed", "error": message }))
            };
            (
                read(&mut processes, "preview.logs"),
                read(&mut processes, "studio.task.logs"),
            )
        };
        let (status, rpc_error) =
            match dev_rpc::request_status(&root.join("build"), Duration::from_millis(200)) {
                Ok(status) => (Some(status), None),
                Err(error) if error.is_not_running() => (None, None),
                Err(error) => (None, Some(error.to_string())),
            };
        Self {
            preview,
            task,
            status,
            rpc_error,
        }
    }

    fn preview_running(&self) -> bool {
        self.preview["running"] == true
    }
    fn task_running(&self) -> bool {
        self.task["running"] == true
    }

    fn preview_label(&self) -> String {
        if let Some(status) = &self.status {
            if let Some(diagnostic) = &status.last_diagnostic {
                return format!("Preview update failed: {}", diagnostic.message);
            }
            let owner = if self.preview_running() {
                ""
            } else {
                " · opened outside this project window"
            };
            let phase = match status.phase {
                DevPhase::Starting => "Preview starting",
                DevPhase::Compiling => "Preview compiling",
                DevPhase::Applying => "Preview applying changes",
                DevPhase::Ready => "Preview ready",
                DevPhase::Failed => "Preview update failed",
                DevPhase::Stopping => "Preview stopping",
            };
            return format!("{phase} · revision {}{owner}", status.applied_revision);
        }
        if let Some(error) = &self.rpc_error {
            return format!("Preview connection failed: {error}");
        }
        if self.preview_running() {
            return "Starting preview…".to_owned();
        }
        if self.preview["phase"] == "failed" {
            return format!(
                "Preview exited · code {} · see Output",
                self.preview["exit_code"]
            );
        }
        "Preview closed".to_owned()
    }

    fn output(&self) -> String {
        let mut sections = Vec::new();
        for (label, process) in [("Preview", &self.preview), ("Task", &self.task)] {
            if process["managed"] != true {
                continue;
            }
            let command = process["args"]
                .as_array()
                .map(|args| {
                    args.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            sections.push(format!(
                "{label}: studio {command}\nStatus: {} · exit {}",
                process["phase"].as_str().unwrap_or("unknown"),
                process["exit_code"]
            ));
            for stream in ["stdout", "stderr"] {
                if let Some(text) = process[stream]["text"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                {
                    // The process service retains bounded logs; the UI also bounds text layout.
                    let start = text.char_indices().rev().nth(8191).map_or(0, |(ix, _)| ix);
                    sections.push(format!("{stream}:\n{}", &text[start..]));
                }
            }
        }
        if sections.is_empty() {
            "No command output yet".to_owned()
        } else {
            sections.join("\n\n")
        }
    }
}

pub(crate) struct ProjectControls {
    root: Option<PathBuf>,
    processes: Arc<Mutex<Processes>>,
    snapshot: ProcessSnapshot,
    generation: u64,
    pending: Option<ProjectCommand>,
    folder_pending: bool,
    message: Option<String>,
    output_open: bool,
    focus: FocusHandle,
}

impl EventEmitter<ProjectControlsEvent> for ProjectControls {}

impl ProjectControls {
    pub(crate) fn new(root: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let shortcut = if cfg!(target_os = "macos") {
            "cmd-o"
        } else {
            "ctrl-o"
        };
        if !cx.has_global::<ProjectKeyBindings>() {
            cx.bind_keys([KeyBinding::new(
                shortcut,
                OpenProjectFolder,
                Some("DesignerProject"),
            )]);
            cx.set_global(ProjectKeyBindings);
        }
        let mut controls = Self {
            root: None,
            processes: Arc::new(Mutex::new(Processes::default())),
            snapshot: ProcessSnapshot::default(),
            generation: 0,
            pending: None,
            folder_pending: false,
            message: None,
            output_open: false,
            focus: cx.focus_handle(),
        };
        if let Some(root) = root {
            controls.set_root(root, cx);
        }
        controls
    }

    pub(crate) fn set_root(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if self.root.as_ref() == Some(&root) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.root = Some(root);
        let old = std::mem::replace(
            &mut self.processes,
            Arc::new(Mutex::new(Processes::default())),
        );
        // Closing a previous project's child may wait for process reaping.
        cx.background_executor()
            .spawn(async move {
                drop(old);
            })
            .detach();
        self.snapshot = ProcessSnapshot::default();
        self.pending = None;
        self.message = None;
        cx.emit(ProjectControlsEvent::PreviewStatus(None));
        self.monitor(cx);
        cx.notify();
    }

    pub(crate) fn open_preview(&mut self, cx: &mut Context<Self>) {
        self.run_command(ProjectCommand::Preview, cx);
    }

    pub(crate) fn show_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.message = Some(message);
        cx.notify();
    }

    pub(crate) fn choose_folder(&mut self, cx: &mut Context<Self>) {
        if self.folder_pending || self.pending.is_some() {
            return;
        }
        self.folder_pending = true;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Studio project".into()),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let selected = selection.await;
            let _ = this.update(cx, |this, cx| {
                this.folder_pending = false;
                match selected {
                    Ok(Ok(Some(paths))) => {
                        if let Some(root) = paths.into_iter().next() {
                            cx.emit(ProjectControlsEvent::OpenFolder(root));
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        this.message = Some(format!("Could not open folder picker: {error}"))
                    }
                    Err(error) => this.message = Some(format!("Folder picker closed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_command(&mut self, command: ProjectCommand, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if self.pending.is_some() {
            return;
        }
        self.pending = Some(command);
        self.message = Some(command.description().to_owned());
        let generation = self.generation;
        let processes = self.processes.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (result, snapshot) = cx
                .background_executor()
                .spawn(async move {
                    let result = processes
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .call(command.tool(), &json!({}), &root)
                        .unwrap();
                    (result, ProcessSnapshot::read(&processes, &root))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.pending = None;
                this.message = result.err().map(|(_, message)| message);
                if this.message.is_some() {
                    this.output_open = true;
                }
                this.apply_snapshot(snapshot, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn monitor(&self, cx: &mut Context<Self>) {
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some((root, processes))) = this.update(cx, |this, _| {
                    (this.generation == generation)
                        .then(|| (this.root.clone().unwrap(), this.processes.clone()))
                }) else {
                    break;
                };
                let snapshot = cx
                    .background_executor()
                    .spawn(async move { ProcessSnapshot::read(&processes, &root) })
                    .await;
                let current = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    this.apply_snapshot(snapshot, cx);
                    true
                });
                if !current.is_ok_and(|current| current) {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
        })
        .detach();
    }

    fn apply_snapshot(&mut self, snapshot: ProcessSnapshot, cx: &mut Context<Self>) {
        if self.snapshot == snapshot {
            return;
        }
        if self.snapshot.status != snapshot.status {
            cx.emit(ProjectControlsEvent::PreviewStatus(snapshot.status.clone()));
        }
        if snapshot.preview["phase"] == "failed" || snapshot.task["phase"] == "failed" {
            self.output_open = true;
        }
        self.snapshot = snapshot;
        cx.notify();
    }
}

impl Render for ProjectControls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let linked = self.root.is_some();
        let pending = self.pending.is_some();
        let connected = self.snapshot.status.is_some();
        let task_running = self.snapshot.task_running();
        let mut buttons = div().flex().flex_wrap().items_center().gap_2().child(
            Button::new("project-open-folder")
                .label("Open folder…")
                .outline()
                .disabled(pending || self.folder_pending)
                .on_click(cx.listener(|this, _, _, cx| this.choose_folder(cx))),
        );
        for (id, label, command, disabled) in [
            (
                "project-preview",
                "Open preview…",
                ProjectCommand::Preview,
                !linked || connected || self.snapshot.preview_running(),
            ),
            (
                "project-restart",
                "Restart preview",
                ProjectCommand::Restart,
                !connected || task_running,
            ),
            (
                "project-stop",
                "Stop preview",
                ProjectCommand::Stop,
                !self.snapshot.preview_running(),
            ),
            (
                "project-check",
                "Check",
                ProjectCommand::Check,
                !linked || task_running,
            ),
            (
                "project-build",
                "Build",
                ProjectCommand::Build,
                !linked || task_running,
            ),
            (
                "project-cancel",
                "Cancel task",
                ProjectCommand::Cancel,
                !task_running,
            ),
        ] {
            buttons = buttons.child(
                Button::new(id)
                    .label(label)
                    .outline()
                    .disabled(pending || disabled)
                    .on_click(cx.listener(move |this, _, _, cx| this.run_command(command, cx))),
            );
        }
        let preview_label = if linked {
            self.snapshot.preview_label()
        } else {
            "Open a project folder to start its preview".to_owned()
        };
        let task_label = if self.snapshot.task["managed"] == true {
            format!(
                "Task {}",
                self.snapshot.task["phase"].as_str().unwrap_or("idle")
            )
        } else {
            String::new()
        };
        div()
            .id("project-command-bar")
            .key_context("DesignerProject")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenProjectFolder, _, cx| this.choose_folder(cx)))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.root.as_ref().map_or_else(
                        || "Project folder".to_owned(),
                        |root| root.display().to_string(),
                    )),
            )
            .child(buttons)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("project-preview-state")
                            .role(Role::Status)
                            .aria_label(preview_label.clone())
                            .child(preview_label),
                    )
                    .child(
                        div()
                            .id("project-task-state")
                            .role(Role::Status)
                            .aria_label(task_label.clone())
                            .child(task_label),
                    )
                    .child(
                        Button::new("project-output-toggle")
                            .label(if self.output_open {
                                "Hide output"
                            } else {
                                "Output"
                            })
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.output_open = !this.output_open;
                                cx.notify();
                            })),
                    ),
            )
            .children(self.message.clone().map(|message| {
                div()
                    .id("project-command-message")
                    .role(Role::Status)
                    .child(message)
            }))
            .when(self.output_open, |element| {
                element.child(
                    div()
                        .id("project-command-output")
                        .h_40()
                        .overflow_scroll()
                        .text_xs()
                        .font_family("monospace")
                        .child(self.snapshot.output()),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};

    #[gpui_kit::test]
    fn folder_picker_cancellation_preserves_project_and_disables_unlinked_commands(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let mut controls = None;
        let window = cx.open_window(size(px(1000.), px(600.)), |window, cx| {
            let view = cx.new(|cx| ProjectControls::new(None, cx));
            controls = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("project-preview").disabled(), Some(true));
            assert_eq!(window.find("project-check").disabled(), Some(true));
            window.click("project-preview", cx);
            window.click("project-open-folder", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|options| {
            assert!(options.directories);
            assert!(!options.files && !options.multiple);
            None
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let controls = controls.as_ref().unwrap().read(cx);
            assert!(controls.root.is_none());
            assert!(!controls.folder_pending);
            assert!(controls.pending.is_none());
        });
        // The project region routes the same command from its desktop shortcut.
        cx.update_window(window.into(), |_, window, cx| {
            let focus = controls.as_ref().unwrap().read(cx).focus.clone();
            window.focus(&focus, cx);
            window.render_frame(cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-o"
                } else {
                    "ctrl-o"
                },
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response(|_| None);
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn folder_picker_sends_selected_folder_to_the_project_owner(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut controls = None;
        let window = cx.open_window(size(px(1000.), px(600.)), |window, cx| {
            let view = cx.new(|cx| ProjectControls::new(None, cx));
            controls = Some(view.clone());
            Root::new(view, window, cx)
        });
        let controls = controls.unwrap();
        let selected = Arc::new(Mutex::new(None));
        let observed = selected.clone();
        let _subscription = controls.update(cx, |_, cx| {
            cx.subscribe(&controls, move |_, _, event, _| {
                if let ProjectControlsEvent::OpenFolder(root) = event {
                    *observed.lock().unwrap() = Some(root.clone());
                }
            })
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("project-open-folder", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let folder = tempfile::tempdir().unwrap();
        cx.simulate_path_prompt_response(|_| Some(vec![folder.path().to_path_buf()]));
        cx.run_until_parked();
        assert_eq!(selected.lock().unwrap().as_deref(), Some(folder.path()));
        // The authenticated owner must admit the folder before any launch occurs.
        cx.update(|cx| assert!(controls.read(cx).root.is_none()));
    }

    #[cfg(unix)]
    #[gpui_kit::test]
    fn toolbar_launches_cli_shows_diagnostics_and_stops_owned_preview(cx: &mut TestAppContext) {
        use std::{fs, os::unix::fs::PermissionsExt, time::Instant};
        let project = tempfile::tempdir().unwrap();
        fs::write(
            project.path().join("app.studio"),
            "<Text id=\"hello\">Hello</Text>",
        )
        .unwrap();
        let cli = project.path().join("test-studio");
        fs::write(&cli, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\nif [ \"$1\" = dev ]; then exec sleep 30; fi\nprintf 'source syntax error' >&2\nexit 10\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        cx.update(gpui_kit::init);
        let mut controls = None;
        let window = cx.open_window(size(px(1100.), px(700.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = ProjectControls::new(Some(project.path().to_path_buf()), cx);
                view.processes = Arc::new(Mutex::new(Processes::with_cli(cli)));
                view
            });
            controls = Some(view.clone());
            Root::new(view, window, cx)
        });
        let controls = controls.unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("project-preview", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let pid = cx.update(|cx| {
            let view = controls.read(cx);
            assert!(view.snapshot.preview_running());
            assert!(view.snapshot.status.is_none(), "spawn is not readiness");
            assert_eq!(view.snapshot.preview["args"][0], "dev");
            assert_eq!(
                view.snapshot.preview["args"][1],
                project.path().to_str().unwrap()
            );
            view.snapshot.preview["pid"].as_u64().unwrap()
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("project-preview").disabled(), Some(true));
            window.click("project-check", cx);
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            let complete = cx.update(|cx| {
                let view = controls.read(cx);
                let processes = view.processes.clone();
                let root = view.root.clone().unwrap();
                let snapshot = ProcessSnapshot::read(&processes, &root);
                let complete = snapshot.task["output_complete"] == true;
                controls.update(cx, |view, cx| view.apply_snapshot(snapshot, cx));
                complete
            });
            if complete {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        cx.update(|cx| {
            let view = controls.read(cx);
            assert_eq!(view.snapshot.task["exit_code"], 10);
            assert!(view.output_open);
            assert!(view.snapshot.output().contains("source syntax error"));
            assert!(
                view.snapshot
                    .output()
                    .contains(project.path().to_str().unwrap())
            );
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("project-stop", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert!(!controls.read(cx).snapshot.preview_running()));
        assert_eq!(
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap()),
                None
            ),
            Err(nix::errno::Errno::ESRCH)
        );

        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("project-preview", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let old_pid = cx.update(|cx| controls.read(cx).snapshot.preview["pid"].as_u64().unwrap());
        let next = tempfile::tempdir().unwrap();
        controls.update(cx, |view, cx| view.set_root(next.path().to_path_buf(), cx));
        cx.run_until_parked();
        cx.update(|cx| {
            let view = controls.read(cx);
            assert_eq!(view.root.as_deref(), Some(next.path()));
            assert!(!view.snapshot.preview_running());
            assert!(view.snapshot.status.is_none());
        });
        assert_eq!(
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(i32::try_from(old_pid).unwrap()),
                None
            ),
            Err(nix::errno::Errno::ESRCH)
        );
    }
}
