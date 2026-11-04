//! Static browser exports compiled from the same Studio project as native bundles.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use studio_script::ir::{StudioModule, TemplateNode};

use super::{BuildDiagnostic, BuildFailure, BuildPhase, BuildRequest};
use crate::studio_project::RoutedProject;

const WEB_CONTRACT: &str = include_str!("../../../../sdk/web/contract.json");

/// Compile an authored Studio project to a self-contained static browser directory.
///
/// # Errors
/// Returns a structured compilation/export error while preserving the previous export.
pub fn build_web(request: &BuildRequest) -> Result<PathBuf, BuildFailure> {
    let _guard = super::LockGuard::acquire(request)?;
    super::validate(request)?;
    let project = compose(request)?;
    validate_kinds(&project.module)?;
    let manifest = std::fs::read(request.project_root.join("manifest.json")).map_err(io_error)?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).map_err(io_error)?;
    if manifest
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|caps| !caps.is_empty())
    {
        return Err(failure(
            "BUILD_WEB_CAPABILITIES",
            "the static browser host does not expose native capabilities",
        ));
    }
    let emitted =
        studio_script::assemblyscript::emit_routed_studio(&project.module, &project.routing)
            .map_err(|error| failure(error.code, error.message))?;
    super::compile_studio_output(request, "web", &emitted.assembly_source)?;
    let assets = super::read_assets(request, &project.assets()?)?;
    let core = core_directory()?;
    let stage = request
        .project_root
        .join("build")
        .join(format!(".web-stage-{}", std::process::id()));
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(io_error)?;
    }
    std::fs::create_dir_all(&stage).map_err(io_error)?;
    let result = export(request, &project, &manifest, &assets, &core, &stage);
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(error);
    }
    commit_export(request, &stage)
}

fn compose(request: &BuildRequest) -> Result<RoutedProject, BuildFailure> {
    if let Some(project) = crate::studio_project::compile_routes(&request.project_root)? {
        return Ok(project);
    }
    crate::studio_project::ensure_generated_assets_available(&request.project_root)?;
    let entry = super::studio_entry(request).ok_or_else(|| {
        failure(
            "BUILD_WEB_SOURCE",
            "web target requires authored .studio sources",
        )
    })?;
    let relative = entry.strip_prefix(&request.project_root).unwrap_or(&entry);
    let name = entry
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    let composed =
        studio_script::project::compile_component(&request.project_root, relative, &[], name)
            .map_err(|errors| {
                failure(
                    "BUILD_WEB_SOURCE",
                    errors
                        .into_iter()
                        .map(|error| format!("{}: {}", error.path, error.diagnostic.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })?;
    let mut project = RoutedProject::single(
        composed,
        relative.to_string_lossy().replace('\\', "/"),
        name.to_owned(),
    );
    "/".clone_into(&mut project.routing.routes[0].path);
    Ok(project)
}

fn validate_kinds(module: &StudioModule) -> Result<(), BuildFailure> {
    let contract: serde_json::Value = serde_json::from_str(WEB_CONTRACT).map_err(io_error)?;
    let kinds = contract["kinds"]
        .as_object()
        .expect("compiled web contract");
    let common = contract["common"]
        .as_array()
        .expect("compiled common web props");
    let mut stack: Vec<_> = module
        .components
        .iter()
        .flat_map(|component| &component.template)
        .collect();
    while let Some(node) = stack.pop() {
        match node {
            TemplateNode::Component {
                kind,
                props,
                children,
                ..
            } => {
                let Some(properties) = kinds
                    .get(&kind.to_ascii_lowercase())
                    .and_then(serde_json::Value::as_array)
                else {
                    return Err(failure(
                        "BUILD_WEB_UNSUPPORTED_KIND",
                        format!("`{kind}` is not supported by the web target"),
                    ));
                };
                for prop in props {
                    if !properties
                        .iter()
                        .chain(common)
                        .any(|name| name.as_str() == Some(&prop.name))
                    {
                        return Err(failure(
                            "BUILD_WEB_UNSUPPORTED_PROPERTY",
                            format!("`{kind}.{}` is not supported by the web target", prop.name),
                        ));
                    }
                }
                stack.extend(children);
            }
            TemplateNode::If {
                consequent,
                alternate,
                ..
            } => {
                stack.extend(consequent);
                stack.extend(alternate);
            }
            TemplateNode::Each { body, fallback, .. } => {
                stack.extend(body);
                stack.extend(fallback);
            }
            _ => {}
        }
    }
    Ok(())
}

fn core_directory() -> Result<PathBuf, BuildFailure> {
    if let Some(path) = std::env::var_os("STUDIO_WEB_CORE_DIR") {
        return require_core(PathBuf::from(path));
    }
    let core = super::repo_root().join(".scratch/web-core");
    let output = Command::new("bun")
        .arg(super::repo_root().join("scripts/build-web-core.ts"))
        .current_dir(super::repo_root())
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(failure(
            "BUILD_WEB_CORE",
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    require_core(core)
}

fn require_core(path: PathBuf) -> Result<PathBuf, BuildFailure> {
    for name in ["studio_web_core.js", "studio_web_core_bg.wasm"] {
        if !path.join(name).is_file() {
            return Err(failure(
                "BUILD_WEB_CORE",
                format!("missing host bridge {}", path.join(name).display()),
            ));
        }
    }
    Ok(path)
}

fn export(
    request: &BuildRequest,
    project: &RoutedProject,
    manifest: &serde_json::Value,
    assets: &BTreeMap<String, Vec<u8>>,
    core: &Path,
    stage: &Path,
) -> Result<(), BuildFailure> {
    let source = super::repo_root().join("sdk/web");
    let output = Command::new("bun")
        .arg("build")
        .arg(source.join("app.ts"))
        .arg(source.join("worker.ts"))
        .args([
            "--target",
            "browser",
            "--format",
            "esm",
            "--external",
            "./core/*",
            "--outdir",
        ])
        .arg(stage)
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(failure(
            "BUILD_WEB_BUNDLE",
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    for name in ["index.html", "style.css"] {
        std::fs::copy(source.join(name), stage.join(name)).map_err(io_error)?;
    }
    std::fs::create_dir_all(stage.join("core")).map_err(io_error)?;
    for name in ["studio_web_core.js", "studio_web_core_bg.wasm"] {
        std::fs::copy(core.join(name), stage.join("core").join(name)).map_err(io_error)?;
    }
    std::fs::copy(
        request.project_root.join("build/module.wasm"),
        stage.join("module.wasm"),
    )
    .map_err(io_error)?;
    for (path, bytes) in assets {
        let destination = stage.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        std::fs::write(destination, bytes).map_err(io_error)?;
    }
    let config = serde_json::json!({"schemaVersion":1,"title":manifest.get("name").and_then(serde_json::Value::as_str).unwrap_or("Studio"),"routing":project.routing,"timers":project.timers,"assets":assets.keys().collect::<Vec<_>>()});
    std::fs::write(
        stage.join("web-config.json"),
        serde_json::to_vec_pretty(&config).map_err(io_error)?,
    )
    .map_err(io_error)?;
    std::fs::write(stage.join("_redirects"), "/* /index.html 200\n").map_err(io_error)?;
    for route in &project.routing.routes {
        if route.path != "/" && !route.path.contains([':', '*']) {
            let directory = stage.join(route.path.trim_start_matches('/'));
            std::fs::create_dir_all(&directory).map_err(io_error)?;
            std::fs::copy(stage.join("index.html"), directory.join("index.html"))
                .map_err(io_error)?;
        }
    }
    Ok(())
}

fn commit_export(request: &BuildRequest, stage: &Path) -> Result<PathBuf, BuildFailure> {
    let destination = request.project_root.join("build/web");
    let backup = request.project_root.join("build/.web-previous");
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(io_error)?;
    }
    if destination.exists() {
        std::fs::rename(&destination, &backup).map_err(io_error)?;
    }
    if let Err(error) = std::fs::rename(stage, &destination) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, &destination);
        }
        return Err(io_error(error));
    }
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(io_error)?;
    }
    Ok(destination)
}

fn io_error(error: impl std::fmt::Display) -> BuildFailure {
    failure("BUILD_WEB_IO", error.to_string())
}
fn failure(code: impl Into<String>, message: impl Into<String>) -> BuildFailure {
    BuildFailure::new(
        BuildPhase::Compile,
        vec![BuildDiagnostic::error(BuildPhase::Compile, code, message)],
    )
}
