//! The phased build pipeline: validate → compile → package.
//!
//! Every failure is a [`BuildFailure`] naming its phase with stable diagnostic
//! codes; packaging goes through `studio_package::pack_bundle` and the bundle
//! is written atomically so failures never damage existing output.
pub mod diagnostics;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use studio_package::{inspect_template, ArchivePolicy, TemplateFiles};
use studio_package::{pack_bundle, pack_template, PackError, PackInput, PackMode};

pub use diagnostics::{
    asc_diagnostics, package_code, BuildDiagnostic, BuildFailure, BuildPhase, EXIT_CANCELLED,
    EXIT_SUCCESS,
};

/// Development signing seed shared with the example packager
/// (`scripts/build-example.ts` used `Buffer.alloc(32, 7)`).
const EXAMPLE_DEV_SEED: [u8; 32] = [7; 32];

/// One resolved build request.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    /// Display/admission name (manifest id stem for path-form projects).
    pub example: String,
    /// Project root containing `assembly/`, `manifest.json`, and `asconfig.json`.
    pub project_root: PathBuf,
    /// Destination bundle path (`<project_root>/build/<example>.studio`).
    pub output_bundle: PathBuf,
}

impl BuildRequest {
    /// Resolve a CLI example argument: either a name under `examples/` or a
    /// project directory path (relative to the current directory).
    ///
    /// # Errors
    ///
    /// Returns a usage-phase failure when the argument resolves to nothing.
    pub fn resolve(argument: &str) -> Result<Self, BuildFailure> {
        let candidate = Path::new(argument);
        if candidate.is_dir() {
            let name = candidate
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_owned());
            let root = candidate.to_path_buf();
            return Ok(Self {
                output_bundle: root.join("build").join(format!("{name}.studio")),
                example: name,
                project_root: root,
            });
        }
        let root = repo_root().join("examples").join(argument);
        if root.is_dir() {
            // Canonicalize: `repo_root` carries `..` segments that bloat
            // every derived path (notably the reload socket, which must fit
            // in SUN_LEN) and pollute log lines.
            let root = root.canonicalize().unwrap_or(root);
            return Ok(Self {
                output_bundle: root.join("build").join(format!("{argument}.studio")),
                example: argument.to_owned(),
                project_root: root,
            });
        }
        Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_USAGE_EXAMPLE_UNKNOWN",
                format!(
                    "unknown example {argument:?}: no directory at {}",
                    root.display()
                ),
            )],
        ))
    }
}

/// Run the full pipeline for one request, serializing concurrent builds of
/// the same project through a PID lockfile next to the bundle output.
///
/// # Errors
///
/// Returns the phase-owned [`BuildFailure`] for the first failing phase.
pub fn build_example(request: &BuildRequest) -> Result<PathBuf, BuildFailure> {
    let _guard = LockGuard::acquire(request)?;
    validate(request)?;
    let staged = compile(request)?;
    package(request, &staged)
}

/// Exclusive, stale-tolerant lockfile for one project's build directory.
struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    fn acquire(request: &BuildRequest) -> Result<Self, BuildFailure> {
        let build_dir = request
            .output_bundle
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| request.project_root.join("build"));
        std::fs::create_dir_all(&build_dir).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Io,
                vec![BuildDiagnostic::error(
                    BuildPhase::Io,
                    "BUILD_IO_LOCK",
                    format!("create {}: {error}", build_dir.display()),
                )],
            )
        })?;
        let path = build_dir.join(format!(".{}.build-lock", request.example));
        let pid = std::process::id().to_string();
        for attempt in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    let _ = file.write_all(pid.as_bytes());
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing = std::fs::read_to_string(&path)
                        .unwrap_or_default()
                        .trim()
                        .to_owned();
                    let stale = existing
                        .parse::<u32>()
                        .map(|holder| !Path::new(&format!("/proc/{holder}")).exists())
                        .unwrap_or(true);
                    if stale && attempt == 0 {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    return Err(BuildFailure::new(
                        BuildPhase::Validate,
                        vec![BuildDiagnostic::error(
                            BuildPhase::Validate,
                            "BUILD_VALIDATE_CONCURRENT",
                            format!(
                                "another studio build holds {} (pid {existing})",
                                path.display()
                            ),
                        )],
                    ));
                }
                Err(error) => {
                    return Err(BuildFailure::new(
                        BuildPhase::Io,
                        vec![BuildDiagnostic::error(
                            BuildPhase::Io,
                            "BUILD_IO_LOCK",
                            format!("acquire {}: {error}", path.display()),
                        )],
                    ));
                }
            }
        }
        unreachable!("lock acquisition loop always returns");
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn validate(request: &BuildRequest) -> Result<(), BuildFailure> {
    let mut diagnostics = Vec::new();
    let root = &request.project_root;
    if !root.is_dir() {
        diagnostics.push(BuildDiagnostic::error(
            BuildPhase::Validate,
            "BUILD_VALIDATE_EXAMPLE_MISSING",
            format!("project directory {} does not exist", root.display()),
        ));
    }
    // Studio-entry projects compile `.studio` sources instead of handwritten
    // AssemblyScript; they need no `assembly/index.ts`.
    if studio_entry(request).is_none() && !root.join("assembly/index.ts").is_file() {
        diagnostics.push(BuildDiagnostic::error(
            BuildPhase::Validate,
            "BUILD_VALIDATE_SOURCE_MISSING",
            format!(
                "expected AssemblyScript entry at {}",
                root.join("assembly/index.ts").display()
            ),
        ));
    }
    if !root.join("manifest.json").is_file() {
        diagnostics.push(BuildDiagnostic::error(
            BuildPhase::Validate,
            "BUILD_VALIDATE_MANIFEST_MISSING",
            format!(
                "expected bundle manifest at {}",
                root.join("manifest.json").display()
            ),
        ));
    }
    let asc = asc_path();
    if !asc.is_file() {
        diagnostics.push(BuildDiagnostic::error(
            BuildPhase::Validate,
            "BUILD_VALIDATE_ASC_MISSING",
            format!(
                "assemblyscript compiler not found at {} (run `bun install --frozen-lockfile`)",
                asc.display()
            ),
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(BuildFailure::new(BuildPhase::Validate, diagnostics))
    }
}

fn compile(request: &BuildRequest) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
    if let Some(entry) = studio_entry(request) {
        compile_studio_entry(request, &entry)?;
        return Ok(BTreeMap::new());
    }
    let staged = collect_lucide(request)?;
    generate_routes(request).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_COMPILE_ROUTES",
                error.to_string(),
            )],
        )
    })?;
    run_asc(request)?;
    Ok(staged)
}

/// A `.studio` entry project: `app.studio` in the project root, else the
/// single `components/*.studio` or `routes/*.studio` file. Multi-entry
/// projects need the module-graph composer (STUDIO340 territory) and are not
/// claimed here.
fn studio_entry(request: &BuildRequest) -> Option<PathBuf> {
    let root = &request.project_root;
    let app = root.join("app.studio");
    if app.is_file() {
        return Some(app);
    }
    let mut entries = Vec::new();
    for dir in ["components", "routes"] {
        let base = root.join(dir);
        let Ok(read) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in read.filter_map(Result::ok) {
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "studio")
            {
                entries.push(entry.path());
            }
        }
    }
    if entries.len() == 1 {
        entries.pop()
    } else {
        None
    }
}

/// Compile one `.studio` entry: adapter → validator → IR → AssemblyScript,
/// then ASC straight to the module wasm consumed by packaging.
fn compile_studio_entry(request: &BuildRequest, entry: &Path) -> Result<(), BuildFailure> {
    let source = std::fs::read_to_string(entry).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_COMPILE_STUDIO_READ",
                format!("read {}: {error}", entry.display()),
            )],
        )
    })?;
    let filename = entry
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "entry.studio".to_owned());
    let stem = entry
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| request.example.clone());
    let module =
        studio_script::compile_studio(&source, &filename, &stem).map_err(|diagnostics| {
            BuildFailure::new(
                BuildPhase::Compile,
                diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        let mut record = BuildDiagnostic::error(
                            BuildPhase::Compile,
                            diagnostic.code,
                            diagnostic.message,
                        );
                        record.line = Some(diagnostic.span.start.line as u64);
                        record.column = Some(diagnostic.span.start.column as u64);
                        record
                    })
                    .collect(),
            )
        })?;
    let emitted = studio_script::assemblyscript::emit_studio(&module).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                error.code,
                error.message,
            )],
        )
    })?;
    let build_dir = request.project_root.join("build");
    std::fs::create_dir_all(&build_dir).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_COMPILE_STUDIO_OUTPUT",
                format!("create {}: {error}", build_dir.display()),
            )],
        )
    })?;
    let generated = build_dir.join(format!("{stem}.generated.ts"));
    write_stable(&generated, &emitted.assembly_source).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_COMPILE_STUDIO_OUTPUT",
                format!("write {}: {error}", generated.display()),
            )],
        )
    })?;
    // The manifest policy admits exactly `module.wasm` as the entry.
    let wasm = build_dir.join("module.wasm");
    let output = Command::new("bun")
        .arg(asc_path())
        .arg(&generated)
        .arg("--outFile")
        .arg(&wasm)
        .arg("--target")
        .arg("release")
        .arg("--maximumMemory")
        .arg(memory_pages(request).to_string())
        .current_dir(&request.project_root)
        .output()
        .map_err(|error| {
            BuildFailure::new(
                BuildPhase::Compile,
                vec![BuildDiagnostic::error(
                    BuildPhase::Compile,
                    "BUILD_COMPILE_ASC_LAUNCH",
                    format!("failed to launch asc: {error}"),
                )],
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(BuildFailure::new(
        BuildPhase::Compile,
        asc_diagnostics(&format!("{stdout}{stderr}")),
    ))
}

/// Write a generated file only when its content changed (watcher hygiene).
fn write_stable(path: &Path, content: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    std::fs::write(path, content)
}

fn run_asc(request: &BuildRequest) -> Result<(), BuildFailure> {
    let output = Command::new("bun")
        .arg(asc_path())
        .arg("assembly/index.ts")
        .arg("--config")
        .arg("asconfig.json")
        .arg("--target")
        .arg("release")
        .arg("--maximumMemory")
        .arg(memory_pages(request).to_string())
        .current_dir(&request.project_root)
        .output()
        .map_err(|error| {
            BuildFailure::new(
                BuildPhase::Compile,
                vec![BuildDiagnostic::error(
                    BuildPhase::Compile,
                    "BUILD_COMPILE_ASC_LAUNCH",
                    format!("failed to launch asc: {error}"),
                )],
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(BuildFailure::new(
        BuildPhase::Compile,
        asc_diagnostics(&format!("{stdout}{stderr}")),
    ))
}

fn package(
    request: &BuildRequest,
    staged: &BTreeMap<String, Vec<u8>>,
) -> Result<PathBuf, BuildFailure> {
    let manifest_bytes =
        std::fs::read(request.project_root.join("manifest.json")).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_MANIFEST_READ",
                    format!("read manifest: {error}"),
                )],
            )
        })?;
    let module_path = compiled_module_path(request);
    let module_bytes = std::fs::read(&module_path).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_MODULE_MISSING",
                format!("read compiled module {}: {error}", module_path.display()),
            )],
        )
    })?;
    let assets = read_assets(request, staged)?;
    // The on-disk manifest is never rewritten (staged icons are build
    // outputs), so pack against the effective manifest: declared entries
    // plus staged keys, sorted to match the supplied asset order.
    let manifest = effective_manifest(&manifest_bytes, assets.keys())?;
    let bytes = pack_bundle(PackInput {
        manifest,
        module: module_bytes,
        assets,
        mode: PackMode::Signed(EXAMPLE_DEV_SEED),
    })
    .map_err(pack_failure)?;
    write_atomically(request, &bytes)?;
    Ok(request.output_bundle.clone())
}

/// Merge staged asset keys into an in-memory copy of the project manifest
/// so the packer sees exactly the supplied asset set.
fn effective_manifest<'a>(
    manifest_bytes: &[u8],
    keys: impl Iterator<Item = &'a String>,
) -> Result<Vec<u8>, BuildFailure> {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(manifest_bytes).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_MANIFEST_READ",
                    format!("parse manifest: {error}"),
                )],
            )
        })?;
    let assets: Vec<String> = keys.cloned().collect();
    manifest["assets"] =
        serde_json::Value::Array(assets.into_iter().map(serde_json::Value::String).collect());
    serde_json::to_vec(&manifest).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_MANIFEST_READ",
                format!("encode effective manifest: {error}"),
            )],
        )
    })
}

/// Resolve the compiled module path: `asconfig.json`'s release `outFile`
/// when declared, otherwise the manifest entry under `build/` when that file
/// exists (Studio entry projects emit `module.wasm`), otherwise the unique
/// `*.wasm` in the build directory.
fn compiled_module_path(request: &BuildRequest) -> PathBuf {
    if let Ok(text) = std::fs::read_to_string(request.project_root.join("asconfig.json")) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(out_file) = config["targets"]["release"]["outFile"].as_str() {
                return request.project_root.join(out_file);
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(request.project_root.join("manifest.json")) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(entry) = manifest["entry"].as_str() {
                let candidate = request.project_root.join("build").join(entry);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    let build_dir = request.project_root.join("build");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&build_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    match candidates.len() {
        1 => candidates.remove(0),
        _ => request
            .project_root
            .join("build")
            .join(format!("{}.wasm", request.example)),
    }
}

/// Read manifest-declared assets from the project, then overlay staged icon
/// bytes (collected icons win on key collision; identical bytes when an
/// override committed the same file the manifest already declares).
fn read_assets(
    request: &BuildRequest,
    staged: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(request.project_root.join("manifest.json")).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_MANIFEST_READ",
                    format!("read manifest: {error}"),
                )],
            )
        })?,
    )
    .map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_MANIFEST_READ",
                format!("decode manifest: {error}"),
            )],
        )
    })?;
    let mut assets = BTreeMap::new();
    if let Some(declared) = manifest["assets"].as_array() {
        for entry in declared {
            let Some(relative) = entry.as_str() else {
                continue;
            };
            let path = request.project_root.join(relative);
            let bytes = std::fs::read(&path).map_err(|error| {
                BuildFailure::new(
                    BuildPhase::Package,
                    vec![BuildDiagnostic::error(
                        BuildPhase::Package,
                        "BUILD_PACKAGE_ASSET_MISSING",
                        format!("read declared asset {relative}: {error}"),
                    )],
                )
            })?;
            assets.insert(relative.to_owned(), bytes);
        }
    }
    for (name, bytes) in staged {
        assets.insert(name.clone(), bytes.clone());
    }
    Ok(assets)
}

#[allow(clippy::needless_pass_by_value)]
fn pack_failure(error: PackError) -> BuildFailure {
    let (code, message) = match &error {
        PackError::Manifest(inner) => (
            package_code("MANIFEST", &format!("{:?}", inner.code())),
            inner.to_string(),
        ),
        PackError::Integrity(inner) => (
            package_code("INTEGRITY", &format!("{:?}", inner.code())),
            inner.to_string(),
        ),
        PackError::Archive(inner) => (
            package_code("ARCHIVE", &format!("{:?}", inner.code())),
            inner.to_string(),
        ),
        PackError::AssetMismatch => (
            "BUILD_PACKAGE_ASSET_MISMATCH".to_owned(),
            "declared bundle assets do not match supplied assets".to_owned(),
        ),
    };
    BuildFailure::new(
        BuildPhase::Package,
        vec![BuildDiagnostic::error(BuildPhase::Package, code, message)],
    )
}

fn write_atomically(request: &BuildRequest, bytes: &[u8]) -> Result<(), BuildFailure> {
    write_atomically_to(
        &request.output_bundle,
        &request.example.replace('/', "_"),
        bytes,
    )
}

fn write_atomically_to(
    destination: &std::path::Path,
    stem: &str,
    bytes: &[u8],
) -> Result<(), BuildFailure> {
    let build_dir = destination
        .parent()
        .ok_or_else(|| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_OUTPUT_INVALID",
                    format!("bundle destination {} has no parent", destination.display()),
                )],
            )
        })?
        .to_path_buf();
    std::fs::create_dir_all(&build_dir).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_OUTPUT_UNWRITABLE",
                format!("create {}: {error}", build_dir.display()),
            )],
        )
    })?;
    let temporary = build_dir.join(format!(".{stem}.studio.tmp"));
    std::fs::write(&temporary, bytes).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_OUTPUT_UNWRITABLE",
                format!("write {}: {error}", temporary.display()),
            )],
        )
    })?;
    std::fs::rename(&temporary, destination).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_PACKAGE_OUTPUT_UNWRITABLE",
                format!("publish bundle to {}: {error}", destination.display()),
            )],
        )
    })
}

/// Publish a shareable template bundle beside the app bundle.
///
/// The project must be a `.studio` source tree carrying its own
/// `template.json`; assembly projects fail closed. The template bundle
/// holds sources only — the consumer rebuilds — and self-verifies
/// through inspection before anything is written.
///
/// # Errors
///
/// Returns usage failures for non-studio projects and missing/invalid
/// manifests, or package failures for archive violations.
pub fn publish_template(request: &BuildRequest) -> Result<PathBuf, BuildFailure> {
    let entry = studio_entry(request).ok_or_else(|| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_TEMPLATE_ASSEMBLY_UNSUPPORTED",
                "template publishing needs a `.studio` source project".to_owned(),
            )],
        )
    })?;
    let template_json =
        std::fs::read(request.project_root.join("template.json")).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_TEMPLATE_MANIFEST_MISSING",
                    format!("template publishing needs template.json: {error}"),
                )],
            )
        })?;
    let manifest_bytes =
        std::fs::read(request.project_root.join("manifest.json")).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_MANIFEST_READ",
                    format!("read manifest: {error}"),
                )],
            )
        })?;
    let entry_source = std::fs::read(&entry).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_COMPILE_STUDIO_READ",
                format!("read {}: {error}", entry.display()),
            )],
        )
    })?;
    // Studio entries stage no icons; declared assets travel with the sources.
    let assets = read_assets(request, &std::collections::BTreeMap::new())?;
    let policy = ArchivePolicy::default();
    let bytes = pack_template(
        &TemplateFiles {
            template_json,
            manifest: manifest_bytes,
            entry_source,
            assets,
        },
        policy,
    )
    .map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_TEMPLATE_ARCHIVE_INVALID",
                format!("pack template bundle: {error}"),
            )],
        )
    })?;
    inspect_template(&bytes, policy).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Package,
            vec![BuildDiagnostic::error(
                BuildPhase::Package,
                "BUILD_TEMPLATE_ARCHIVE_INVALID",
                format!("verify template bundle: {error}"),
            )],
        )
    })?;
    let stem = request
        .output_bundle
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| request.example.clone());
    let destination = request
        .output_bundle
        .parent()
        .map(|parent| parent.join(format!("{stem}.template.studio")))
        .ok_or_else(|| {
            BuildFailure::new(
                BuildPhase::Package,
                vec![BuildDiagnostic::error(
                    BuildPhase::Package,
                    "BUILD_PACKAGE_OUTPUT_INVALID",
                    "bundle destination has no parent".to_owned(),
                )],
            )
        })?;
    write_atomically_to(&destination, &format!("{stem}.template"), &bytes)?;
    Ok(destination)
}

fn asc_path() -> PathBuf {
    repo_root().join("sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js")
}

/// Guest memory pages declared to asc: the manifest `limits.memoryMiB`
/// ceiling in 64 KiB pages. The host module policy requires a declared
/// maximum, so every compiled guest carries the manifest ceiling.
fn memory_pages(request: &BuildRequest) -> u64 {
    let mib = std::fs::read_to_string(request.project_root.join("manifest.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|manifest| manifest["limits"]["memoryMiB"].as_u64())
        .filter(|mib| *mib > 0)
        .unwrap_or(16);
    mib * 16
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Content-stable lucide collection: copies only missing icons and rewrites
/// `manifest.json` only when the declared asset list actually changes.
/// Typed lucide collection: references resolved by parsing, staged under
/// `build/staging/icons/`, manifest never rewritten. Returns staged bytes by
/// asset rel path (`assets/icons/<name>.svg`).
fn collect_lucide(request: &BuildRequest) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
    use crate::assets::{collect_references, resolve_collection, stage_collection};

    let references = collect_references(&request.project_root);
    if references.is_empty() {
        return Ok(BTreeMap::new());
    }
    let package = repo_root().join("node_modules/lucide-static/icons");
    let collection =
        resolve_collection(&request.project_root, &references, &package).map_err(|failure| {
            let mut diagnostic =
                BuildDiagnostic::error(BuildPhase::Compile, failure.code, failure.message);
            if let Some(span) = failure.spans.first() {
                diagnostic.line = Some(span.line);
                diagnostic.column = Some(span.column);
            }
            BuildFailure::new(BuildPhase::Compile, vec![diagnostic])
        })?;
    let staged = stage_collection(&request.project_root, &collection).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Compile,
            vec![BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_ASSET_STAGE_FAILED",
                format!("stage collected icons: {error}"),
            )],
        )
    })?;
    let mut bytes = BTreeMap::new();
    for (name, icon) in &collection.entries {
        let _ = staged.get(&format!("assets/icons/{name}.svg"));
        bytes.insert(format!("assets/icons/{name}.svg"), icon.bytes.clone());
    }
    Ok(bytes)
}

/// Scan a project's `routes/` directory into sorted route paths.
pub(crate) fn scan_routes(project: &std::path::Path) -> Vec<String> {
    let routes_dir = project.join("routes");
    let mut routes = Vec::new();
    if !routes_dir.is_dir() {
        return routes;
    }
    let walk = walkdir::WalkDir::new(&routes_dir);
    for entry in walk.into_iter().filter_map(Result::ok) {
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "ts")
        {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&routes_dir) else {
            continue;
        };
        let mut route = format!(
            "/{}",
            rel.with_extension("").to_string_lossy().replace('\\', "/")
        );
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
        routes.push(route);
    }
    routes.sort();
    routes
}

/// Content-stable route generation: `routes.generated.ts` is rewritten only
/// when its rendered content differs from what is on disk. The registry
/// renderer keeps the legacy constants byte-identical and appends the table.
pub fn generate_routes(request: &BuildRequest) -> anyhow::Result<()> {
    let dir = &request.project_root;
    if !dir.join("routes").exists() {
        return Ok(());
    }
    let registry = crate::routes::build_registry(dir).map_err(|diagnostics| {
        let summary = diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::anyhow!("route registry failed: {summary}")
    })?;
    let out = dir.join("assembly/routes.generated.ts");
    let content = crate::routes::render_module(&registry);
    if std::fs::read_to_string(&out).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    std::fs::write(out, content)?;
    println!("routes: generated {} routes", registry.entries.len());
    Ok(())
}
