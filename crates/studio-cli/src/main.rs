use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll, Wake, Waker};
use studio_design::{
    Actor, ActorId, ActorKind, CommandBatch, CommandOutcome, DefaultDesignerSession, DesignerQuery,
    DesignerQueryResult, DesignerSession, InMemoryDesignerPersistence, OperationId, StudioDesign,
    UndoGroupId,
};
use studio_script::{format, parse, Diagnostic, Severity, CODE_NON_CANONICAL_FORMAT};

use studio_cli::build::{BuildDiagnostic, BuildPhase};
use studio_cli::{build, watch};

/// JSON exit-code family for check/fmt validation failures (validate family).
const EXIT_VALIDATION: u8 = 10;
/// Exit-code family for io/environment failures.
const EXIT_IO: u8 = 13;

#[derive(Parser)]
#[command(
    name = "studio",
    version,
    about = "Studio CLI — unified bundler, dev, preview"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum BuildTarget {
    #[default]
    Native,
    Web,
}

#[derive(Subcommand)]
enum Commands {
    /// Open the native development host with content-hashed IR/Wasm hot reload.
    Dev {
        #[arg(default_value = "pos-desktop")]
        example: String,
        #[arg(long, default_value = "5123")]
        port: u16,
    },
    /// Build a project through the validate/compile/package phases into a
    /// byte-identical, atomically written bundle.
    Build {
        /// Example name under examples/ or a project directory path.
        #[arg(default_value = "pos-desktop")]
        example: String,
        /// Also emit a shareable template bundle beside the app bundle.
        #[arg(long)]
        as_template: bool,
        /// Compile to a native package or a static browser app.
        #[arg(long, value_enum, default_value_t = BuildTarget::Native)]
        target: BuildTarget,
    },
    /// Preview built bundle through the runtime host in development mode.
    Preview {
        #[arg(default_value = "pos-desktop")]
        example: String,
    },
    /// Restart a running dev session through its reload channel.
    RestartSession {
        /// Example name under examples/ or a project directory path.
        #[arg(default_value = "pos-desktop")]
        example: String,
    },
    /// Generate protocol schemas + AssemblyScript bindings
    Generate,
    /// Validate `.studio` files and emit one structured JSON diagnostic per finding
    Check {
        /// Files or directories to check. Directories are searched recursively.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
    },
    /// Rewrite `.studio` files using the canonical Studio Script printer
    Fmt {
        /// Files or directories to format. Directories are searched recursively.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// Check formatting without modifying files.
        #[arg(long)]
        check: bool,
    },
    /// Replay typed Designer command batches from a JSON document.
    Replay {
        /// JSON file to replay, or stdin when omitted.
        path: Option<PathBuf>,
    },
    /// Scaffold a modern-Script project from the template gallery.
    New {
        /// Project directory name (created in the working directory).
        name: Option<String>,
        /// Gallery template, `owner/repo`, git URL, or direct link.
        #[arg(short, long, default_value = "blank")]
        template: String,
        /// Overwrite a non-empty destination.
        #[arg(long)]
        force: bool,
        /// List gallery templates and exit.
        #[arg(long)]
        list_templates: bool,
        /// List output format: human text or machine JSON.
        #[arg(long, value_parser = ["text", "json"], default_value = "text")]
        format: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Dev { example, port } => finish(dev(&example, port)),
        Commands::Build {
            example,
            as_template,
            target,
        } => finish(build(&example, as_template, target)),
        Commands::Preview { example } => finish(preview(&example)),
        Commands::RestartSession { example } => finish(restart_session(&example)),
        Commands::Generate => finish(generate()),
        Commands::Check { paths } => match check_studio_files(&paths) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(EXIT_VALIDATION),
            Err(error) => io_exit(error),
        },
        Commands::Fmt { paths, check } => match format_studio_files(&paths, check) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(EXIT_VALIDATION),
            Err(error) => io_exit(error),
        },
        Commands::Replay { path } => match replay(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => io_exit(error),
        },
        Commands::New {
            name,
            template,
            force,
            list_templates,
            format,
        } => new_project(name, template, force, list_templates, format),
    }
}

fn finish(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn io_exit(error: anyhow::Error) -> ExitCode {
    BuildDiagnostic::error(BuildPhase::Io, "BUILD_IO", format!("{error:#}")).report();
    ExitCode::from(EXIT_IO)
}

fn build(example: &str, as_template: bool, target: BuildTarget) -> i32 {
    let request = match build::BuildRequest::resolve(example) {
        Ok(request) => request,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    println!("studio build {}", request.example);
    if target == BuildTarget::Web && as_template {
        BuildDiagnostic::error(
            BuildPhase::Usage,
            "BUILD_WEB_TEMPLATE_UNSUPPORTED",
            "web exports do not support --as-template",
        )
        .report();
        return 2;
    }
    let result = match target {
        BuildTarget::Native => build::build_example(&request),
        BuildTarget::Web => build::web::build_web(&request),
    };
    let bundle = match result {
        Ok(bundle) => bundle,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    println!("built {}", bundle.display());
    if as_template {
        return match build::publish_template(&request) {
            Ok(template) => {
                println!("templated {}", template.display());
                build::EXIT_SUCCESS
            }
            Err(failure) => {
                failure.report();
                failure.exit_code()
            }
        };
    }
    build::EXIT_SUCCESS
}

fn restart_session(example: &str) -> i32 {
    let request = match build::BuildRequest::resolve(example) {
        Ok(request) => request,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    let socket = studio_protocol::reload::socket_path(&request.project_root.join("build"));
    watch::restart_session(&socket)
}

fn dev(example: &str, port: u16) -> i32 {
    let request = match build::BuildRequest::resolve(example) {
        Ok(request) => request,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    studio_cli::dev::run(request, port)
}

/// JSON envelope accepted by the headless Designer replay command.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayInput {
    design: StudioDesign,
    batches: Vec<CommandBatch>,
}

/// Deterministic replay report emitted as one JSON object.
#[derive(Debug, serde::Serialize)]
struct ReplayReport {
    outcomes: Vec<CommandOutcome>,
    snapshot: studio_design::StudioDesignSnapshot,
    reopened_snapshot: studio_design::StudioDesignSnapshot,
    deterministic: bool,
}

fn new_project(
    name: Option<String>,
    template: String,
    force: bool,
    list_templates: bool,
    format: String,
) -> ExitCode {
    if list_templates {
        return match studio_cli::new::list_templates() {
            Ok(entries) => {
                if format == "json" {
                    let items: Vec<serde_json::Value> = entries
                        .iter()
                        .map(|(id, meta)| {
                            serde_json::json!({
                                "id": id,
                                "kind": meta.kind,
                                "description": meta.description,
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_owned())
                    );
                } else {
                    for (id, meta) in entries {
                        println!(
                            "{id} [{kind}] {description}",
                            kind = meta.kind,
                            description = meta.description
                        );
                    }
                }
                ExitCode::SUCCESS
            }
            Err(failure) => {
                failure.report();
                finish(failure.exit_code())
            }
        };
    }
    let Some(name) = name else {
        eprintln!("usage: studio new <name> [-t <template>] [--force]");
        return finish(2);
    };
    match studio_cli::new::run(&name, &template, force) {
        Ok(destination) => {
            println!("created {}", destination.display());
            ExitCode::SUCCESS
        }
        Err(failure) => {
            failure.report();
            finish(failure.exit_code())
        }
    }
}

fn replay(path: Option<PathBuf>) -> Result<()> {
    let source = match path {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("read replay input from {}", path.display()))?,
        None => {
            let mut source = String::new();
            std::io::stdin()
                .read_to_string(&mut source)
                .context("read replay input from stdin")?;
            source
        }
    };
    let input: ReplayInput = serde_json::from_str(&source).context("decode replay input")?;
    let first = run_replay(&input)?;
    let second = run_replay(&input)?;
    let report = ReplayReport {
        outcomes: first.0.clone(),
        snapshot: first.1.clone(),
        reopened_snapshot: first.2.clone(),
        deterministic: first == second,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn run_replay(
    input: &ReplayInput,
) -> Result<(
    Vec<CommandOutcome>,
    studio_design::StudioDesignSnapshot,
    studio_design::StudioDesignSnapshot,
)> {
    let persistence = InMemoryDesignerPersistence::default();
    let actor = Actor {
        id: ActorId::new("studio-cli-replay")?,
        kind: ActorKind::Human,
        display_name: "Studio CLI replay".to_owned(),
    };
    let project_id = input.design.project_id.clone();
    let mut session = block_on(DefaultDesignerSession::create(
        persistence.clone(),
        input.design.clone(),
        OperationId::new("studio-cli-replay-create")?,
        actor,
        UndoGroupId::new("studio-cli-replay-create")?,
    ))
    .context("create replay session")?;
    let outcomes = input
        .batches
        .iter()
        .cloned()
        .map(|batch| block_on(session.submit(batch)))
        .collect();
    let current_snapshot = snapshot(&session)?;
    let reopened = block_on(DefaultDesignerSession::open(persistence, &project_id))
        .context("reopen replay session")?;
    let reopened_snapshot = snapshot(&reopened)?;
    Ok((outcomes, current_snapshot, reopened_snapshot))
}

fn snapshot<S: DesignerSession>(session: &S) -> Result<studio_design::StudioDesignSnapshot> {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => Ok(snapshot),
        other => anyhow::bail!("snapshot query returned unexpected result: {other:?}"),
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}

    fn wake_by_ref(self: &Arc<Self>) {}
}

fn studio_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let roots = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    };
    let mut files = Vec::new();
    for root in roots {
        if root.is_file() {
            files.push(root);
            continue;
        }
        if !root.is_dir() {
            anyhow::bail!("path does not exist: {}", root.display());
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|entry| {
                !matches!(
                    entry.file_name().to_str(),
                    Some("build" | "node_modules" | ".git")
                )
            })
        {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "studio")
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn check_studio_files(paths: &[PathBuf]) -> Result<bool> {
    let files = studio_files(paths)?;
    let mut valid = true;
    let mut checked_projects = std::collections::BTreeSet::new();
    for path in files {
        let project_root = path
            .ancestors()
            .skip(1)
            .find(|ancestor| ancestor.join("manifest.json").is_file());
        if let Some(root) = project_root {
            if !root.join("studio.project.json").exists()
                && studio_cli::studio_project::has_file_routes(root)
            {
                if !checked_projects.insert(root.to_path_buf()) {
                    continue;
                }
                match studio_cli::studio_project::compile_routes(root) {
                    Ok(Some(project)) => {
                        for route in project.routing.routes {
                            print_status(&root.join(route.source), true, false);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        error.report();
                        valid = false;
                    }
                }
                continue;
            }
        }
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                print_io_diagnostic(&path, &error);
                valid = false;
                continue;
            }
        };
        // Legacy canonical sources carry the `studio 1` version header;
        // Svelte-shaped sources compile through the Studio Script pipeline.
        if source.trim_start().starts_with("studio 1") {
            match parse(&source) {
                Ok(_) => print_status(&path, true, false),
                Err(error) => {
                    print_diagnostics(&path, &error.diagnostics);
                    valid = false;
                }
            }
            continue;
        }
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "check.studio".to_owned());
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "check".to_owned());
        let result = if let Some(root) = project_root {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            studio_script::project::compile_component(root, relative, &[], &name)
                .map(|compiled| compiled.module)
                .map_err(|errors| errors.into_iter().map(|error| error.diagnostic).collect())
        } else {
            studio_script::compile_studio(&source, &filename, &name)
        };
        match result {
            Ok(_) => print_status(&path, true, false),
            Err(diagnostics) => {
                print_diagnostics(&path, &diagnostics);
                valid = false;
            }
        }
    }
    Ok(valid)
}

fn format_studio_files(paths: &[PathBuf], check_only: bool) -> Result<bool> {
    let files = studio_files(paths)?;
    let mut valid = true;
    for path in files {
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                print_io_diagnostic(&path, &error);
                valid = false;
                continue;
            }
        };
        let canonical = match format(&source) {
            Ok(canonical) => canonical,
            Err(error) => {
                print_diagnostics(&path, &error.diagnostics);
                valid = false;
                continue;
            }
        };
        if source == canonical {
            print_status(&path, true, false);
        } else if check_only {
            let diagnostic = Diagnostic {
                code: CODE_NON_CANONICAL_FORMAT,
                severity: Severity::Error,
                message: "file is not in canonical Studio Script format".to_owned(),
                span: studio_script::Span {
                    start: studio_script::Location {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                    end: studio_script::Location {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                },
            };
            print_diagnostics(&path, &[diagnostic]);
            valid = false;
        } else {
            std::fs::write(&path, canonical)
                .with_context(|| format!("write canonical Studio Script to {}", path.display()))?;
            print_status(&path, true, true);
        }
    }
    Ok(valid)
}

fn print_status(path: &Path, ok: bool, changed: bool) {
    let value = serde_json::json!({
        "path": path.display().to_string(),
        "ok": ok,
        "changed": changed,
    });
    println!("{value}");
}

fn print_diagnostics(path: &Path, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        let value = serde_json::json!({
            "path": path.display().to_string(),
            "code": diagnostic.code,
            "severity": match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            "message": diagnostic.message,
            "line": diagnostic.span.start.line,
            "column": diagnostic.span.start.column,
            "offset": diagnostic.span.start.offset,
        });
        println!("{value}");
    }
}

fn print_io_diagnostic(path: &Path, error: &std::io::Error) {
    let value = serde_json::json!({
        "path": path.display().to_string(),
        "code": "STUDIO_IO",
        "severity": "error",
        "message": error.to_string(),
        "line": 1,
        "column": 1,
        "offset": 0,
    });
    println!("{value}");
}

fn preview(example: &str) -> i32 {
    if Path::new(example).is_file() {
        return studio_cli::dev::preview(Path::new(example));
    }
    let request = match build::BuildRequest::resolve(example) {
        Ok(request) => request,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    let bundle = request.output_bundle.clone();
    if !bundle.is_file() {
        BuildDiagnostic::error(
            BuildPhase::Watch,
            "BUILD_WATCH_BUNDLE_MISSING",
            format!(
                "no bundle at {} — run `studio build` first",
                bundle.display()
            ),
        )
        .report();
        return BuildPhase::Watch.exit_code();
    }
    studio_cli::dev::preview(&bundle)
}

fn generate() -> i32 {
    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("studio-protocol")
        .arg("--bin")
        .arg("generate_schema")
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            BuildDiagnostic::error(
                BuildPhase::Io,
                "BUILD_IO_GENERATE",
                format!("generate_schema exited: {status}"),
            )
            .report();
            return BuildPhase::Io.exit_code();
        }
        Err(error) => {
            BuildDiagnostic::error(
                BuildPhase::Io,
                "BUILD_IO_GENERATE",
                format!("launch generate_schema: {error}"),
            )
            .report();
            return BuildPhase::Io.exit_code();
        }
    }
    let status = Command::new("bun")
        .arg(build::repo_root().join("scripts/generate-protocol.ts"))
        .status();
    match status {
        Ok(status) if status.success() => build::EXIT_SUCCESS,
        Ok(status) => {
            BuildDiagnostic::error(
                BuildPhase::Io,
                "BUILD_IO_GENERATE",
                format!("generate-protocol.ts exited: {status}"),
            )
            .report();
            BuildPhase::Io.exit_code()
        }
        Err(error) => {
            BuildDiagnostic::error(
                BuildPhase::Io,
                "BUILD_IO_GENERATE",
                format!("launch generate-protocol.ts: {error}"),
            )
            .report();
            BuildPhase::Io.exit_code()
        }
    }
}
