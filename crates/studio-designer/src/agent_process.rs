//! CLI child processes owned by one Designer project or MCP session.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

type ToolResult = Result<Value, (String, String)>;
const LOG_BYTES: usize = 64 * 1024;

pub(crate) struct Processes {
    cli: PathBuf,
    runtime_binary: PathBuf,
    preview: Option<ManagedProcess>,
    runtime: Option<ManagedProcess>,
    task: Option<ManagedProcess>,
}

impl Default for Processes {
    fn default() -> Self {
        Self {
            cli: executable("studio"),
            runtime_binary: executable("studio-app"),
            preview: None,
            runtime: None,
            task: None,
        }
    }
}

impl Processes {
    #[cfg(test)]
    pub(crate) fn with_cli(cli: PathBuf) -> Self {
        Self {
            cli,
            ..Self::default()
        }
    }

    pub(crate) fn call(
        &mut self,
        name: &str,
        arguments: &Value,
        root: &Path,
    ) -> Option<ToolResult> {
        Some(match name {
            "preview.start" => self.start_preview(root),
            "preview.stop" => stop(&mut self.preview),
            "preview.logs" => logs(&mut self.preview),
            "preview.restart" => self.start_task(root, "restart-session"),
            "studio.check" => self.start_check(root),
            "studio.build" => self.start_task(root, "build"),
            "studio.task.status" => status(&mut self.task),
            "studio.task.logs" => logs(&mut self.task),
            "studio.task.cancel" => stop(&mut self.task),
            "runtime.start" => self.start_runtime(arguments, root),
            "runtime.status" => status(&mut self.runtime),
            "runtime.logs" => logs(&mut self.runtime),
            "runtime.stop" => stop(&mut self.runtime),
            _ => return None,
        })
    }

    fn start_preview(&mut self, root: &Path) -> ToolResult {
        // An existing host can be controlled through its RPC but is never adopted
        // as a child that this server may terminate.
        match studio_protocol::dev_rpc::request_status(&root.join("build"), Duration::from_secs(2))
        {
            Ok(status) => {
                Ok(json!({ "running": true, "already_running": true, "preview": status }))
            }
            Err(error) if error.is_not_running() => start(
                &mut self.preview,
                &self.cli,
                &["dev".into(), root.into()],
                root,
            ),
            Err(error) => Err(("PREVIEW_UNAVAILABLE".into(), error.to_string())),
        }
    }

    fn start_task(&mut self, root: &Path, command: &str) -> ToolResult {
        start(
            &mut self.task,
            &self.cli,
            &[command.into(), root.into()],
            root,
        )
    }

    fn start_check(&mut self, root: &Path) -> ToolResult {
        let workspace = crate::source_workspace::ProjectSourceWorkspace::open(root)
            .map_err(|error| (error.code().into(), error.to_string()))?;
        let files = workspace
            .list_files()
            .map_err(|error| (error.code().into(), error.to_string()))?;
        let mut args = vec![OsString::from("check")];
        args.extend(
            files
                .iter()
                .filter(|file| {
                    Path::new(&file.path)
                        .extension()
                        .is_some_and(|extension| extension == "studio")
                })
                .map(|file| root.join(&file.path).into_os_string()),
        );
        if args.len() == 1 {
            return Err(error(
                "NO_SOURCE_FILES",
                "no accessible .studio source files to check",
            ));
        }
        start(&mut self.task, &self.cli, &args, root)
    }

    fn start_runtime(&mut self, arguments: &Value, root: &Path) -> ToolResult {
        let relative = arguments
            .get("bundle")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                error(
                    "invalid_arguments",
                    "bundle must be a project-relative .studio path",
                )
            })?;
        let path = Path::new(relative);
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || path
                .extension()
                .is_none_or(|extension| extension != "studio")
        {
            return Err(error(
                "invalid_arguments",
                "bundle must be a normalized project-relative .studio path",
            ));
        }
        let bundle = root.join(path).canonicalize().map_err(io_error)?;
        if !bundle.starts_with(root) || !bundle.is_file() {
            return Err(error(
                "BUNDLE_DENIED",
                "bundle must be a regular file inside the project",
            ));
        }
        start(
            &mut self.runtime,
            &self.runtime_binary,
            &["--bundle".into(), bundle.into()],
            root,
        )
    }
}

fn error(code: &str, message: impl Into<String>) -> (String, String) {
    (code.into(), message.into())
}

fn io_error(error: std::io::Error) -> (String, String) {
    ("PROCESS_IO".into(), error.to_string())
}

fn executable(name: &str) -> PathBuf {
    // Overrides come from the host environment, never from tool arguments.
    let variable = if name == "studio" {
        "STUDIO_CLI_BINARY"
    } else {
        "STUDIO_RUNTIME_BINARY"
    };
    if let Some(path) = std::env::var_os(variable) {
        return PathBuf::from(path);
    }
    let filename = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join(&filename);
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from(filename)
}

fn start(
    slot: &mut Option<ManagedProcess>,
    command: &Path,
    args: &[OsString],
    root: &Path,
) -> ToolResult {
    if let Some(process) = slot {
        if process.poll().map_err(io_error)?.is_none() {
            return Err(error(
                "PROCESS_BUSY",
                "a process is already running in this slot; query its status or stop it first",
            ));
        }
    }
    let process = ManagedProcess::spawn(command, args, root).map_err(io_error)?;
    *slot = Some(process);
    status(slot)
}

fn status(slot: &mut Option<ManagedProcess>) -> ToolResult {
    match slot {
        Some(process) => process.status(),
        None => Ok(json!({ "running": false, "phase": "not_started", "managed": false })),
    }
}

fn logs(slot: &mut Option<ManagedProcess>) -> ToolResult {
    let mut result = status(slot)?;
    if let Some(process) = slot {
        result["stdout"] = process.stdout.snapshot();
        result["stderr"] = process.stderr.snapshot();
    }
    Ok(result)
}

fn stop(slot: &mut Option<ManagedProcess>) -> ToolResult {
    if let Some(process) = slot {
        process.stop().map_err(io_error)?;
    }
    status(slot)
}

#[derive(Default)]
struct LogBuffer {
    bytes: VecDeque<u8>,
    discarded: usize,
}

impl LogBuffer {
    fn append(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
        let excess = self.bytes.len().saturating_sub(LOG_BYTES);
        self.bytes.drain(..excess);
        self.discarded = self.discarded.saturating_add(excess);
    }
}

#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<LogBuffer>>);

impl CapturedLog {
    fn capture(&self, mut reader: impl Read + Send + 'static, done: Arc<AtomicUsize>) {
        let log = self.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => log.0.lock().unwrap().append(&buffer[..count]),
                }
            }
            done.fetch_add(1, Ordering::Release);
        });
    }

    fn snapshot(&self) -> Value {
        let log = self.0.lock().unwrap();
        let bytes: Vec<_> = log.bytes.iter().copied().collect();
        json!({ "text": String::from_utf8_lossy(&bytes), "discarded_bytes": log.discarded })
    }
}

struct ManagedProcess {
    child: Child,
    command: PathBuf,
    args: Vec<OsString>,
    exit: Option<ExitStatus>,
    stdout: CapturedLog,
    stderr: CapturedLog,
    readers_done: Arc<AtomicUsize>,
    stopped: bool,
}

impl ManagedProcess {
    fn spawn(command: &Path, args: &[OsString], root: &Path) -> std::io::Result<Self> {
        let mut launcher = Command::new(command);
        launcher
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            launcher.process_group(0);
        }
        let mut child = launcher.spawn()?;
        let stdout = CapturedLog::default();
        let stderr = CapturedLog::default();
        let readers_done = Arc::new(AtomicUsize::new(0));
        stdout.capture(child.stdout.take().unwrap(), readers_done.clone());
        stderr.capture(child.stderr.take().unwrap(), readers_done.clone());
        Ok(Self {
            child,
            command: command.into(),
            args: args.to_vec(),
            exit: None,
            stdout,
            stderr,
            readers_done,
            stopped: false,
        })
    }

    fn poll(&mut self) -> std::io::Result<Option<ExitStatus>> {
        if self.exit.is_none() {
            self.exit = self.child.try_wait()?;
        }
        Ok(self.exit)
    }

    fn status(&mut self) -> ToolResult {
        let exit = self.poll().map_err(io_error)?;
        Ok(json!({
            "running": exit.is_none(), "managed": true, "pid": self.child.id(),
            "phase": if exit.is_none() { "running" } else if self.stopped { "stopped" } else if exit.is_some_and(|exit| exit.success()) { "succeeded" } else { "failed" },
            "exit_code": exit.and_then(|exit| exit.code()),
            "output_complete": self.readers_done.load(Ordering::Acquire) == 2,
            "command": self.command.to_string_lossy(),
            "args": self.args.iter().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>()
        }))
    }

    fn stop(&mut self) -> std::io::Result<()> {
        if self.poll()?.is_none() {
            #[cfg(unix)]
            {
                use nix::sys::signal::{Signal, killpg};
                use nix::unistd::Pid;
                let pid = i32::try_from(self.child.id()).map_err(std::io::Error::other)?;
                match killpg(Pid::from_raw(pid), Signal::SIGKILL) {
                    Ok(()) | Err(nix::errno::Errno::ESRCH) => {}
                    Err(error) => return Err(std::io::Error::other(error)),
                }
            }
            #[cfg(not(unix))]
            self.child.kill()?;
            self.exit = Some(self.child.wait()?);
            self.stopped = true;
        }
        Ok(())
    }
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) fn tools() -> Vec<Value> {
    let mut tools = Vec::new();
    for (name, description, read_only) in [
        (
            "studio.check",
            "Start studio check for this project's sources. Poll studio.task.status and read studio.task.logs for JSON diagnostics.",
            false,
        ),
        (
            "studio.build",
            "Start studio build for this project, producing a release bundle. Poll studio.task.status and read studio.task.logs; this may invoke the project's compiler.",
            false,
        ),
        (
            "studio.task.status",
            "Return completion and exit code for the most recent MCP-started check, build, or preview restart.",
            true,
        ),
        (
            "studio.task.logs",
            "Read bounded stdout and stderr tails for the most recent CLI task, including compiler diagnostics.",
            true,
        ),
        (
            "studio.task.cancel",
            "Terminate the CLI task started by this MCP session.",
            false,
        ),
        (
            "preview.start",
            "Start studio dev in this project folder. If a preview already answers RPC, report it. Poll preview.status for readiness and preview.logs for startup errors.",
            false,
        ),
        (
            "preview.stop",
            "Terminate only the preview process started by this MCP session. Externally started previews are left running.",
            false,
        ),
        (
            "preview.restart",
            "Start studio restart-session for this project's live preview, resetting guest state. Poll studio.task.status and logs.",
            false,
        ),
        (
            "preview.logs",
            "Read bounded stdout/stderr tails and process status for the preview started by this MCP session.",
            true,
        ),
        (
            "runtime.status",
            "Return process status and exit code for the release runtime launched by this MCP session. This does not inspect the runtime UI.",
            true,
        ),
        (
            "runtime.logs",
            "Read bounded stdout and stderr tails for the release runtime launched by this MCP session.",
            true,
        ),
        (
            "runtime.stop",
            "Terminate the release runtime launched by this MCP session.",
            false,
        ),
    ] {
        tools.push(json!({
            "name": name, "description": description,
            "annotations": { "readOnlyHint": read_only, "destructiveHint": !read_only, "openWorldHint": false },
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }));
    }
    tools.push(json!({
        "name": "runtime.start",
        "description": "Launch studio-app with an existing signed .studio bundle inside this project. Publisher trust must already be provisioned; failures appear in runtime.logs. No dev mode is enabled.",
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false },
        "inputSchema": { "type": "object", "properties": { "bundle": { "type": "string", "description": "Project-relative release bundle path, for example build/my-app.studio." } }, "required": ["bundle"], "additionalProperties": false }
    }));
    tools
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::Instant;

    fn script(root: &Path, source: &str) -> PathBuf {
        let path = root.join("fake-studio");
        fs::write(&path, format!("#!/bin/sh\n{source}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn completed(slot: &mut Option<ManagedProcess>) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = logs(slot).unwrap();
            if result["running"] == false && result["output_complete"] == true {
                return result;
            }
            assert!(Instant::now() < deadline, "child did not finish: {result}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn cli_checks_only_accessible_sources_and_reports_failure_and_logs() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("app.studio"), "bad source").unwrap();
        fs::create_dir(root.path().join("build")).unwrap();
        fs::write(root.path().join("build/release.studio"), "archive").unwrap();
        let cli = script(
            root.path(),
            "printf '%s\\n' \"$PWD\" \"$@\"; printf 'diagnostic' >&2; exit 10",
        );
        let mut processes = Processes {
            cli,
            ..Processes::default()
        };
        processes
            .call("studio.check", &json!({}), root.path())
            .unwrap()
            .unwrap();
        let result = completed(&mut processes.task);
        assert_eq!(result["phase"], "failed");
        assert_eq!(result["exit_code"], 10);
        let stdout = result["stdout"]["text"].as_str().unwrap();
        assert!(stdout.contains(root.path().to_str().unwrap()));
        assert!(stdout.contains("check\n"));
        assert!(stdout.contains("app.studio"));
        assert!(!stdout.contains("release.studio"));
        assert_eq!(result["stderr"]["text"], "diagnostic");
    }

    #[test]
    fn process_slots_reject_duplicates_and_stop_owned_preview() {
        let root = tempfile::tempdir().unwrap();
        let cli = script(root.path(), "exec sleep 30");
        let mut processes = Processes {
            cli,
            ..Processes::default()
        };
        let started = processes
            .call("preview.start", &json!({}), root.path())
            .unwrap()
            .unwrap();
        assert_eq!(started["running"], true);
        assert_eq!(started["args"][0], "dev");
        let duplicate = processes
            .call("preview.start", &json!({}), root.path())
            .unwrap()
            .unwrap_err();
        assert_eq!(duplicate.0, "PROCESS_BUSY");
        let stopped = processes
            .call("preview.stop", &json!({}), root.path())
            .unwrap()
            .unwrap();
        assert_eq!(stopped["running"], false);
        assert_eq!(stopped["phase"], "stopped");
    }

    #[test]
    fn existing_preview_is_observed_without_taking_process_ownership() {
        use std::io::{BufRead, BufReader, Write};
        use studio_protocol::dev_rpc::{self, DevStatus};
        let root = tempfile::tempdir().unwrap();
        let build = root.path().join("build");
        fs::create_dir(&build).unwrap();
        let socket = dev_rpc::socket_path(&build);
        let listener = dev_rpc::bind_endpoint(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request).unwrap();
            let request: Value = serde_json::from_str(&request).unwrap();
            let response =
                json!({ "jsonrpc": "2.0", "id": request["id"], "result": DevStatus::default() });
            writeln!(stream, "{response}").unwrap();
        });
        let mut processes = Processes::default();
        let result = processes
            .call("preview.start", &json!({}), root.path())
            .unwrap()
            .unwrap();
        assert_eq!(result["already_running"], true);
        assert!(processes.preview.is_none());
        let stopped = processes
            .call("preview.stop", &json!({}), root.path())
            .unwrap()
            .unwrap();
        assert_eq!(stopped["managed"], false);
        server.join().unwrap();
    }

    #[test]
    fn release_launch_is_confined_and_uses_runtime_bundle_argument() {
        let root = tempfile::tempdir().unwrap();
        let binary = script(root.path(), "printf '%s\\n' \"$@\"");
        fs::write(root.path().join("app.studio"), "bundle").unwrap();
        let external = tempfile::NamedTempFile::new().unwrap();
        symlink(external.path(), root.path().join("escape.studio")).unwrap();
        let mut processes = Processes {
            runtime_binary: binary,
            ..Processes::default()
        };
        for path in ["../app.studio", "/app.studio", "escape.studio", "app.ts"] {
            assert!(
                processes
                    .call("runtime.start", &json!({"bundle": path}), root.path())
                    .unwrap()
                    .is_err()
            );
        }
        processes
            .call(
                "runtime.start",
                &json!({"bundle": "app.studio"}),
                root.path(),
            )
            .unwrap()
            .unwrap();
        let result = completed(&mut processes.runtime);
        assert_eq!(result["phase"], "succeeded");
        assert_eq!(result["args"][0], "--bundle");
        assert_eq!(
            result["args"][1],
            root.path().join("app.studio").to_str().unwrap()
        );
    }

    #[test]
    fn output_capture_keeps_a_bounded_tail_and_counts_discarded_bytes() {
        let mut log = LogBuffer::default();
        log.append(&vec![b'a'; LOG_BYTES]);
        log.append(b"tail");
        assert_eq!(log.bytes.len(), LOG_BYTES);
        assert_eq!(log.discarded, 4);
        assert_eq!(
            log.bytes.iter().rev().take(4).copied().collect::<Vec<_>>(),
            b"liat"
        );
    }

    #[test]
    fn dropping_a_running_child_terminates_and_reaps_it() {
        let root = tempfile::tempdir().unwrap();
        let cli = script(root.path(), "exec sleep 30");
        let process = ManagedProcess::spawn(&cli, &[], root.path()).unwrap();
        let pid = nix::unistd::Pid::from_raw(i32::try_from(process.child.id()).unwrap());
        drop(process);
        assert_eq!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        );
    }
}
