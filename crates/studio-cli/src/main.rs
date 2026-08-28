use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll, Wake, Waker};
use studio_design::{
    Actor, ActorId, ActorKind, CommandBatch, CommandOutcome, DefaultDesignerSession, DesignerQuery,
    DesignerQueryResult, DesignerSession, InMemoryDesignerPersistence, OperationId, StudioDesign,
    UndoGroupId,
};
use studio_script::{format, parse, Diagnostic, Severity, CODE_NON_CANONICAL_FORMAT};

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

#[derive(Subcommand)]
enum Commands {
    /// Watch assembly/routes/assets, rebuild wasm & pack on change (HMR placeholder)
    Dev {
        #[arg(default_value = "pos-desktop")]
        example: String,
        #[arg(long, default_value = "5123")]
        port: u16,
    },
    /// Build example: asc + lucide collect + routes gen + pack
    Build {
        #[arg(default_value = "pos-desktop")]
        example: String,
    },
    /// Preview built bundle in studio-app host
    Preview {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Dev { example, port } => dev(&example, port),
        Commands::Build { example } => build(&example),
        Commands::Preview { example } => preview(&example),
        Commands::Generate => generate(),
        Commands::Check { paths } => {
            if check_studio_files(&paths)? {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Commands::Fmt { paths, check } => {
            if format_studio_files(&paths, check)? {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Commands::Replay { path } => replay(path),
    }
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
        for entry in walkdir::WalkDir::new(root) {
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
    for path in files {
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                print_io_diagnostic(&path, &error);
                valid = false;
                continue;
            }
        };
        match parse(&source) {
            Ok(_) => print_status(&path, true, false),
            Err(error) => {
                print_diagnostics(&path, &error.diagnostics);
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn example_dir(example: &str) -> PathBuf {
    repo_root().join("examples").join(example)
}

fn collect_lucide(example: &str) -> Result<()> {
    let dir = example_dir(example);
    let assembly = dir.join("assembly");
    if !assembly.exists() {
        return Ok(());
    }
    // Find Icon("name") and lucide imports
    let mut used = std::collections::BTreeSet::new();
    // Simpler: scan for iconNode and Icon with second arg as name
    let re2 = regex::Regex::new(r#"iconNode\s*\([^,]+,\s*"([^"]+)""#).unwrap();
    let re3 = regex::Regex::new(r#"Icon\s*\([^,]+,\s*"([^"]+)""#).unwrap();
    for entry in walkdir::WalkDir::new(&assembly) {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "ts").unwrap_or(false) {
            let content = std::fs::read_to_string(entry.path())?;
            // Find iconNode("id", "name") -> capture name
            for cap in re2.captures_iter(&content) {
                used.insert(cap[1].to_string());
            }
            for cap in re3.captures_iter(&content) {
                used.insert(cap[1].to_string());
            }
        }
    }
    if used.is_empty() {
        return Ok(());
    }
    let dest = dir.join("assets/icons");
    std::fs::create_dir_all(&dest)?;
    let src_base = repo_root().join("node_modules/lucide-static/icons");
    // fallback to vendor or download on demand
    for name in used {
        let src = src_base.join(format!("{}.svg", name));
        let dst = dest.join(format!("{}.svg", name));
        if src.exists() && !dst.exists() {
            std::fs::copy(&src, &dst).with_context(|| format!("copy {}", name))?;
            println!("lucide: + assets/icons/{}.svg", name);
        }
    }
    // Update manifest assets lex-sorted
    let manifest_path = dir.join("manifest.json");
    let manifest_str = std::fs::read_to_string(&manifest_path)?;
    let mut manifest: serde_json::Value = serde_json::from_str(&manifest_str)?;
    let mut assets: Vec<String> = manifest["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    for entry in std::fs::read_dir(&dest)? {
        let entry = entry?;
        let rel = format!("assets/icons/{}", entry.file_name().to_string_lossy());
        if !assets.contains(&rel) {
            assets.push(rel);
        }
    }
    assets.sort();
    manifest["assets"] =
        serde_json::Value::Array(assets.into_iter().map(serde_json::Value::String).collect());
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn generate_routes(example: &str) -> Result<()> {
    let dir = example_dir(example);
    let routes_dir = dir.join("routes");
    if !routes_dir.exists() {
        return Ok(());
    }
    let mut routes = Vec::new();
    for entry in walkdir::WalkDir::new(&routes_dir) {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "ts").unwrap_or(false) {
            let rel = entry.path().strip_prefix(&routes_dir).unwrap();
            let mut route = format!(
                "/{}",
                rel.with_extension("").to_string_lossy().replace("\\", "/")
            );
            // pos.ts -> /pos, index.ts -> /
            if route == "/index" {
                route = "/".to_string();
            } else if route.ends_with("/index") {
                route = route.trim_end_matches("index").to_string();
                if route.ends_with('/') && route.len() > 1 {
                    route.pop();
                }
                if route.is_empty() {
                    route = "/".to_string();
                }
            }
            println!("routes scan: {:?} -> {}", rel, route);
            routes.push(route);
        }
    }
    routes.sort();
    let out = dir.join("assembly/routes.generated.ts");
    let content = format!(
        "// Generated from routes/ — do not edit\n{}",
        routes
            .iter()
            .map(|r| format!(
                "export const route_{} = \"{}\";",
                r.replace(['/', '-'], "_").trim_matches('_'),
                r
            ))
            .collect::<Vec<_>>()
            .join("\n")
            + &format!(
                "\nexport const declaredRoutes = [{}];\n",
                routes
                    .iter()
                    .map(|r| format!("\"{}\"", r))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
    );
    std::fs::write(out, content)?;
    println!("routes: generated {} routes", routes.len());
    Ok(())
}

fn run_asc(example: &str) -> Result<()> {
    let dir = example_dir(example);
    let asc = repo_root().join("sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js");
    let status = Command::new("bun")
        .arg(asc)
        .arg("assembly/index.ts")
        .arg("--config")
        .arg("asconfig.json")
        .arg("--target")
        .arg("release")
        .current_dir(&dir)
        .status()
        .context("run asc")?;
    if !status.success() {
        anyhow::bail!("asc failed");
    }
    Ok(())
}

fn pack(example: &str) -> Result<()> {
    let root = repo_root();
    let status = Command::new("bun")
        .arg(root.join("scripts/build-example.ts"))
        .arg(example)
        .current_dir(&root)
        .status()
        .context("pack")?;
    if !status.success() {
        anyhow::bail!("pack failed");
    }
    Ok(())
}

fn build(example: &str) -> Result<()> {
    println!("studio build {}", example);
    let _ = collect_lucide(example);
    let _ = generate_routes(example);
    run_asc(example)?;
    pack(example)?;
    println!("built examples/{}/build/{}.studio", example, example);
    Ok(())
}

fn dev(example: &str, port: u16) -> Result<()> {
    println!(
        "studio dev {} on :{} watching assembly/ routes/ assets/",
        example, port
    );
    build(example)?;
    let dir = example_dir(example);
    let bundle = dir.join("build").join(format!("{}.studio", example));
    let _child = Command::new("target/debug/studio-app")
        .arg("--dev")
        .arg(&bundle)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("GALLIUM_DRIVER", "llvmpipe")
        .spawn()
        .ok();
    println!(
        "preview launched (if studio-app built) — polling for changes (notify feature pending)"
    );
    let watch_dirs = [dir.join("assembly"), dir.join("routes"), dir.join("assets")];
    let mut last_mtimes: std::collections::HashMap<PathBuf, std::time::SystemTime> =
        Default::default();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut changed = false;
        for watch in &watch_dirs {
            if !watch.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(watch) {
                let Ok(entry) = entry else { continue };
                let Ok(meta) = entry.metadata() else { continue };
                let Ok(mtime) = meta.modified() else { continue };
                let path = entry.path().to_path_buf();
                if last_mtimes.get(&path) != Some(&mtime) {
                    if last_mtimes.contains_key(&path) {
                        changed = true;
                    }
                    last_mtimes.insert(path, mtime);
                }
            }
        }
        if changed {
            println!("change detected — rebuilding");
            if let Err(e) = build(example) {
                eprintln!("build error: {:#}", e);
            } else {
                println!("rebuild ok — refresh studio-app window");
            }
        }
    }
}

fn preview(example: &str) -> Result<()> {
    let bundle = example_dir(example)
        .join("build")
        .join(format!("{}.studio", example));
    let status = Command::new("target/debug/studio-app")
        .arg("--dev")
        .arg(bundle)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("GALLIUM_DRIVER", "llvmpipe")
        .status()?;
    if !status.success() {
        anyhow::bail!("preview failed");
    }
    Ok(())
}

fn generate() -> Result<()> {
    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("studio-protocol")
        .arg("--bin")
        .arg("generate_schema")
        .status()?;
    if !status.success() {
        anyhow::bail!("generate_schema failed");
    }
    let status = Command::new("bun")
        .arg(repo_root().join("scripts/generate-protocol.ts"))
        .status()?;
    if !status.success() {
        anyhow::bail!("generate-protocol.ts failed");
    }
    Ok(())
}
