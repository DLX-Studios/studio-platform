//! Content-addressed development artifacts. No archive or signing stage is involved.
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use studio_script::ir::StudioModule;

use crate::build::{self, BuildDiagnostic, BuildFailure, BuildPhase, BuildRequest};

#[derive(Clone)]
pub(crate) enum Code {
    Ir(Arc<StudioModule>),
    Wasm(Arc<Vec<u8>>),
}

pub(crate) struct Artifact {
    pub code: Code,
    pub code_hash: String,
    pub manifest: Vec<u8>,
    pub assets: BTreeMap<String, Vec<u8>>,
    pub screen_source_paths: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Inputs {
    pub files: BTreeMap<PathBuf, [u8; 32]>,
    pub code_hash: String,
}

pub(crate) struct DevCompiler {
    request: BuildRequest,
    cache: BTreeMap<String, Code>,
    cancelled: Arc<AtomicBool>,
}

pub(crate) fn failure(message: impl Into<String>) -> BuildFailure {
    BuildFailure::new(
        BuildPhase::Compile,
        vec![BuildDiagnostic::error(
            BuildPhase::Compile,
            "DEV_COMPILE",
            message,
        )],
    )
}

impl DevCompiler {
    pub fn new(mut request: BuildRequest) -> Self {
        request.project_root = request
            .project_root
            .canonicalize()
            .unwrap_or(request.project_root);
        Self {
            request,
            cache: BTreeMap::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_cancellation(&mut self, cancelled: Arc<AtomicBool>) {
        self.cancelled = cancelled;
    }

    /// Hash content, file identities, compiler inputs, SDK sources and external relative imports.
    /// File additions/deletions are part of the snapshot; timestamps are never cache keys.
    pub fn inputs(&self) -> Result<Inputs, BuildFailure> {
        let root = &self.request.project_root;
        let sdk = build::repo_root().join("sdk/assemblyscript");
        let mut paths = source_files(root);
        paths.extend(source_files(&sdk.join("assembly")));
        for path in [
            sdk.join("package.json"),
            sdk.join("node_modules/assemblyscript/package.json"),
            build::asc_path(),
            build::repo_root().join("bun.lock"),
        ] {
            if path.is_file() {
                paths.push(path);
            }
        }
        // Follow relative imports outside the project as well. This is invalidation,
        // not a second parser/compiler: the actual frontend still owns syntax admission.
        let imports = regex::Regex::new(r#"(?:\bfrom\s*|\bimport\s*)["']([^"']+)["']"#)
            .expect("static import pattern");
        let mut files = BTreeMap::new();
        let mut code = Sha256::new();
        code.update(b"studio-development-artifact/1\0");
        code.update(env!("CARGO_PKG_VERSION").as_bytes());
        code.update(include_bytes!("compiler.rs"));
        while let Some(path) = paths.pop() {
            let path = path
                .canonicalize()
                .map_err(|error| failure(format!("{}: {error}", path.display())))?;
            if files.contains_key(&path) {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|error| failure(format!("{}: {error}", path.display())))?;
            if path
                .extension()
                .is_some_and(|ext| ext == "ts" || ext == "studio")
            {
                let source = String::from_utf8_lossy(&bytes);
                for capture in imports.captures_iter(&source) {
                    let specifier = &capture[1];
                    if !specifier.starts_with('.') && !Path::new(specifier).is_absolute() {
                        continue;
                    }
                    let imported = path.parent().unwrap_or(root).join(specifier);
                    for candidate in [
                        imported.clone(),
                        imported.with_extension("ts"),
                        imported.with_extension("studio"),
                        imported.join("index.ts"),
                    ] {
                        if candidate.is_file() && !generated(&candidate) {
                            if path
                                .extension()
                                .is_some_and(|extension| extension == "studio")
                            {
                                let resolved = candidate
                                    .canonicalize()
                                    .map_err(|error| failure(error.to_string()))?;
                                if !resolved.starts_with(root) {
                                    return Err(failure(
                                        "Studio component import resolves outside the project root",
                                    ));
                                }
                            }
                            paths.push(candidate);
                            break;
                        }
                    }
                }
            }
            files.insert(path, Sha256::digest(&bytes).into());
        }
        for (path, hash) in &files {
            // Assets/manifest are separately refreshed. Code regeneration depends on
            // sources, route identities, SDK, compiler, and compiler configuration.
            if path.starts_with(root.join("assets"))
                || path.file_name().is_some_and(|name| name == "manifest.json")
            {
                continue;
            }
            code.update(path.to_string_lossy().as_bytes());
            code.update([0]);
            code.update(hash);
        }
        // Memory limits alter the compiled Wasm resource declaration.
        code.update(build::memory_pages(&self.request).to_le_bytes());
        Ok(Inputs {
            files,
            code_hash: code
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
    }

    pub fn compile(&mut self, inputs: &Inputs) -> Result<Artifact, BuildFailure> {
        let declared_screens = super::project::load_screen_entries(&self.request.project_root)?;
        if declared_screens.is_none() {
            if let Some(project) =
                crate::studio_project::compile_routes(&self.request.project_root)?
            {
                let manifest = std::fs::read(self.request.project_root.join("manifest.json"))
                    .map_err(|error| failure(error.to_string()))?;
                studio_package::parse_manifest(
                    &manifest,
                    studio_package::ManifestPolicy::default(),
                )
                .map_err(|error| failure(error.to_string()))?;
                let mut staged = build::collect_lucide(&self.request)?;
                staged.extend(project.assets()?);
                let assets = build::read_assets(&self.request, &staged)?;
                let manifest = build::effective_manifest(&manifest, assets.keys())?;
                let code = if let Some(code) = self.cache.get(&inputs.code_hash) {
                    code.clone()
                } else {
                    let code = Code::Ir(Arc::new(project.module));
                    if self.cache.len() >= 32 {
                        self.cache.clear();
                    }
                    self.cache.insert(inputs.code_hash.clone(), code.clone());
                    code
                };
                let screen_source_paths = project
                    .routing
                    .routes
                    .into_iter()
                    .map(|route| (route.id, route.source))
                    .collect();
                return Ok(Artifact {
                    code,
                    code_hash: inputs.code_hash.clone(),
                    manifest,
                    assets,
                    screen_source_paths,
                });
            }
        }
        let legacy_entry = if declared_screens.is_none() {
            build::studio_entry(&self.request)
        } else {
            None
        };
        let screen_source_paths = if let Some(screens) = &declared_screens {
            screens
                .iter()
                .map(|screen| (screen.id.clone(), screen.source_path.clone()))
                .collect()
        } else if let Some(entry) = &legacy_entry {
            let name = entry
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("app")
                .to_owned();
            let source_path = entry
                .strip_prefix(&self.request.project_root)
                .unwrap_or(entry)
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            BTreeMap::from([(name, source_path)])
        } else {
            BTreeMap::new()
        };
        let single = if let Some(entry) = &legacy_entry {
            let name = entry
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("app");
            let relative = entry
                .strip_prefix(&self.request.project_root)
                .unwrap_or(entry);
            let composed = studio_script::project::compile_component(
                &self.request.project_root,
                relative,
                &[],
                name,
            )
            .map_err(|errors| {
                failure(
                    errors
                        .into_iter()
                        .map(|error| format!("{}: {}", error.path, error.diagnostic.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })?;
            Some(crate::studio_project::RoutedProject::single(
                composed,
                relative.to_string_lossy().replace('\\', "/"),
                name.to_owned(),
            ))
        } else {
            None
        };
        let declared_manifest = std::fs::read(self.request.project_root.join("manifest.json"))
            .map_err(|error| failure(error.to_string()))?;
        studio_package::parse_manifest(
            &declared_manifest,
            studio_package::ManifestPolicy::default(),
        )
        .map_err(|error| failure(error.to_string()))?;
        let mut staged = build::collect_lucide(&self.request)?;
        if let Some(project) = &single {
            crate::studio_project::ensure_generated_assets_available(&self.request.project_root)?;
            let mut generated = project.assets()?;
            if project.timers.pages.is_empty() {
                generated.remove(studio_navigation::project::ROUTING_ASSET);
            }
            staged.extend(generated);
        }
        let assets = build::read_assets(&self.request, &staged)?;
        let manifest = std::fs::read(self.request.project_root.join("manifest.json"))
            .map_err(|error| failure(format!("read manifest: {error}")))?;
        let manifest = build::effective_manifest(&manifest, assets.keys())?;
        let code = if let Some(code) = self.cache.get(&inputs.code_hash) {
            code.clone()
        } else {
            let code = if let Some(screens) = &declared_screens {
                Code::Ir(Arc::new(compile_studio_project(screens)?))
            } else if let Some(project) = single {
                Code::Ir(Arc::new(project.module))
            } else {
                Code::Wasm(Arc::new(self.compile_wasm(&inputs.code_hash)?))
            };
            // Keep a bounded in-memory working set, including versions users may undo to.
            if self.cache.len() >= 32 {
                self.cache.clear();
            }
            self.cache.insert(inputs.code_hash.clone(), code.clone());
            code
        };
        Ok(Artifact {
            code,
            code_hash: inputs.code_hash.clone(),
            manifest,
            assets,
            screen_source_paths,
        })
    }

    fn compile_wasm(&self, hash: &str) -> Result<Vec<u8>, BuildFailure> {
        build::generate_routes(&self.request).map_err(|error| failure(error.to_string()))?;
        let root = &self.request.project_root;
        let output = root.join("build/.studio-dev");
        let cache = output.join("cache");
        std::fs::create_dir_all(&cache).map_err(|error| failure(error.to_string()))?;
        let artifact = cache.join(format!("{hash}.wasm"));
        if artifact.is_file() {
            return std::fs::read(&artifact).map_err(|error| failure(error.to_string()));
        }
        // Publish only complete compiler output. An interrupted session cannot poison a cache key.
        let pending_artifact = cache.join(format!("{hash}.{}.pending.wasm", std::process::id()));
        let entry = root.join("assembly/index.ts");
        if !entry.is_file() {
            return Err(failure("expected app.studio or assembly/index.ts"));
        }
        let dev_sdk = build::repo_root().join("sdk/assemblyscript/assembly/dev.ts");
        let sdk_import = relative_import(&output, &dev_sdk)?;
        let wrapper = output.join("entry.ts");
        std::fs::write(&wrapper, format!(
            "// Generated development entry.\nexport * from \"../../assembly/index\";\nexport {{ studio_dev_snapshot, studio_dev_restore }} from {};\n",
            serde_json::to_string(&sdk_import).map_err(|error| failure(error.to_string()))?,
        )).map_err(|error| failure(error.to_string()))?;
        let mut child = Command::new("bun")
            .arg(build::asc_path())
            .arg(&wrapper)
            .arg("--config")
            .arg(root.join("asconfig.json"))
            .args([
                "--target",
                "debug",
                "--optimizeLevel",
                "0",
                "--shrinkLevel",
                "0",
            ])
            .args(["--use", "STUDIO_DEV=1", "--outFile"])
            .arg(&pending_artifact)
            .arg("--maximumMemory")
            .arg(build::memory_pages(&self.request).to_string())
            .current_dir(root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| failure(format!("launch AssemblyScript: {error}")))?;
        // Drain both pipes concurrently so verbose diagnostics cannot stall cancellation.
        let stdout = child.stdout.take().expect("piped compiler stdout");
        let stderr = child.stderr.take().expect("piped compiler stderr");
        let drain = |mut pipe: Box<dyn std::io::Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).map(|_| bytes)
            })
        };
        let stdout = drain(Box::new(stdout));
        let stderr = drain(Box::new(stderr));
        let status = loop {
            if self.cancelled.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout.join();
                let _ = stderr.join();
                let _ = std::fs::remove_file(&pending_artifact);
                return Err(failure("development compilation cancelled"));
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| failure(error.to_string()))?
            {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        let stdout = stdout
            .join()
            .map_err(|_| failure("compiler stdout reader failed"))?
            .map_err(|error| failure(error.to_string()))?;
        let stderr = stderr
            .join()
            .map_err(|_| failure("compiler stderr reader failed"))?
            .map_err(|error| failure(error.to_string()))?;
        if !status.success() {
            let _ = std::fs::remove_file(&pending_artifact);
            let diagnostics = format!(
                "{}{}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            return Err(BuildFailure::new(
                BuildPhase::Compile,
                build::asc_diagnostics(&diagnostics),
            ));
        }
        let module =
            std::fs::read(&pending_artifact).map_err(|error| failure(error.to_string()))?;
        std::fs::rename(&pending_artifact, &artifact)
            .map_err(|error| failure(error.to_string()))?;
        Ok(module)
    }
}

fn compile_studio_project(
    screens: &[super::project::ScreenEntry],
) -> Result<StudioModule, BuildFailure> {
    let mut components = Vec::with_capacity(screens.len());
    let mut version = None;
    for screen in screens {
        let module = compile_studio_source(&screen.source, &screen.source_path, &screen.id)?;
        if let Some(expected) = version {
            if module.version != expected {
                return Err(failure(
                    "declared screens use incompatible Studio IR versions",
                ));
            }
        } else {
            version = Some(module.version);
        }
        let mut component = module.components.into_iter().next().ok_or_else(|| {
            failure(format!(
                "screen `{}` compiled without a component",
                screen.id
            ))
        })?;
        component.id.clone_from(&screen.id);
        component.name.clone_from(&screen.id);
        components.push(component);
    }
    let exports = components
        .iter()
        .map(|component| component.name.clone())
        .collect();
    Ok(StudioModule {
        version: version.unwrap_or(studio_script::ir::STUDIO_IR_VERSION),
        id: "studio-project".to_owned(),
        imports: Vec::new(),
        exports,
        components,
        dependencies: Vec::new(),
    })
}

fn compile_studio_source(
    source: &str,
    source_path: &str,
    component_name: &str,
) -> Result<StudioModule, BuildFailure> {
    studio_script::compile_studio(source, source_path, component_name).map_err(|diagnostics| {
        BuildFailure::new(
            BuildPhase::Compile,
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    BuildDiagnostic::error(
                        BuildPhase::Compile,
                        diagnostic.code,
                        format!("{source_path}: {}", diagnostic.message),
                    )
                    .at(
                        diagnostic.span.start.line as u64,
                        diagnostic.span.start.column as u64,
                    )
                })
                .collect(),
        )
    })
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| {
            let allowed_name = !matches!(
                entry.file_name().to_str(),
                Some("build" | "node_modules" | ".git")
            );
            let within_root = entry
                .path()
                .canonicalize()
                .is_ok_and(|canonical| canonical.starts_with(&canonical_root));
            allowed_name && within_root
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.canonicalize()
                .is_ok_and(|canonical| canonical.starts_with(&canonical_root))
        })
        .filter(|path| !generated(path))
        .collect()
}

fn generated(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().contains(".generated."))
}

fn relative_import(from: &Path, to: &Path) -> Result<String, BuildFailure> {
    let from = from
        .canonicalize()
        .map_err(|error| failure(error.to_string()))?;
    let to = to
        .canonicalize()
        .map_err(|error| failure(error.to_string()))?;
    let from: Vec<_> = from.components().collect();
    let to_path = to.clone();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        // AssemblyScript accepts absolute module paths, including another Windows drive.
        return Ok(to_path
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/"));
    }
    let mut relative = PathBuf::from(".");
    for _ in shared..from.len() {
        relative.push("..");
    }
    for component in &to[shared..] {
        if let Component::Normal(part) = component {
            relative.push(part);
        }
    }
    Ok(relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (tempfile::TempDir, DevCompiler) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("assets")).unwrap();
        std::fs::write(
            root.join("manifest.json"),
            include_bytes!("../../../../examples/starter/manifest.json"),
        )
        .unwrap();
        std::fs::write(root.join("app.studio"), "<Text id=\"hello\">Hello</Text>").unwrap();
        let request = BuildRequest {
            example: "dev".into(),
            project_root: root.to_path_buf(),
            output_bundle: root.join("build/dev.studio"),
        };
        (directory, DevCompiler::new(request))
    }

    #[test]
    fn standalone_imported_component_source_links_keep_the_actual_file() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::create_dir(root.join("components")).unwrap();
        std::fs::write(
            root.join("components/Heading.studio"),
            "<Text id=\"heading\">Heading</Text>",
        )
        .unwrap();
        std::fs::write(root.join("app.studio"), "<script lang=\"ts\">import Heading from './components/Heading.studio';</script><Heading id=\"hero\" />").unwrap();
        let artifact = compiler.compile(&compiler.inputs().unwrap()).unwrap();
        let links = artifact
            .assets
            .get(crate::studio_project::SOURCE_MAP_ASSET)
            .expect("composed standalone sources need actual source maps");
        let links: serde_json::Value = serde_json::from_slice(links).unwrap();
        assert_eq!(
            links["app"]["hero--heading"]["path"],
            "components/Heading.studio"
        );
    }

    #[test]
    fn studio_import_hashing_rejects_external_sources_before_reading_them() {
        let (directory, compiler) = project();
        let outside = tempfile::tempdir().unwrap();
        let dependency = outside.path().join("External.studio");
        std::fs::write(&dependency, "<Text>Outside</Text>").unwrap();
        std::fs::write(
            directory.path().join("app.studio"),
            format!(
                "<script lang=\"ts\">import External from '{}';</script><External />",
                dependency.display()
            ),
        )
        .unwrap();
        assert!(
            compiler.inputs().is_err(),
            "Studio source imports may not leave the project while hashing"
        );
    }

    #[test]
    fn ir_edits_and_assets_are_hashed_without_creating_a_bundle_or_invoking_asc() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        let first = compiler.inputs().unwrap();
        let artifact = compiler.compile(&first).unwrap();
        assert!(matches!(artifact.code, Code::Ir(_)));
        assert!(!root.join("build/dev.studio").exists());
        assert!(!root.join("build/module.wasm").exists());
        std::fs::write(root.join("assets/logo.svg"), "one").unwrap();
        let assets = compiler.inputs().unwrap();
        assert_ne!(first, assets);
        assert_eq!(
            first.code_hash, assets.code_hash,
            "asset updates do not recompile code"
        );
        std::fs::write(root.join("app.studio"), "<Text id=\"hello\">Edited</Text>").unwrap();
        let edited = compiler.inputs().unwrap();
        assert_ne!(assets.code_hash, edited.code_hash);
        compiler.compile(&edited).unwrap();
        assert!(!root.join("build/dev.studio").exists());
        std::fs::write(root.join("app.studio"), "<Text id=\"hello\">Hello</Text>").unwrap();
        assert_eq!(compiler.inputs().unwrap().code_hash, first.code_hash);
    }

    #[test]
    fn declared_project_screens_compile_in_order_with_stable_ids_and_source_paths() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::create_dir(root.join("screens")).unwrap();
        std::fs::write(
            root.join("screens/home.studio"),
            "<script lang=\"ts\">let count = $state(1);</script><Text id=\"title\">{count}</Text>",
        )
        .unwrap();
        std::fs::write(
            root.join("screens/settings.studio"),
            "<script lang=\"ts\">let count = $state(2);</script><Text id=\"title\">{count}</Text>",
        )
        .unwrap();
        std::fs::write(
            root.join("studio.project.json"),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"screens/home.studio"},{"id":"settings","source":"screens/settings.studio"}]}"#,
        )
        .unwrap();

        let inputs = compiler.inputs().unwrap();
        assert!(inputs.files.contains_key(&root.join("studio.project.json")));
        let artifact = compiler.compile(&inputs).unwrap();
        let Code::Ir(module) = artifact.code else {
            panic!("declared Studio screens compile to IR");
        };
        assert_eq!(
            module
                .components
                .iter()
                .map(|component| component.id.as_str())
                .collect::<Vec<_>>(),
            vec!["home", "settings"]
        );
        assert_eq!(
            module
                .exports
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["home", "settings"]
        );
        assert_eq!(
            artifact.screen_source_paths,
            BTreeMap::from([
                ("home".to_owned(), "screens/home.studio".to_owned()),
                ("settings".to_owned(), "screens/settings.studio".to_owned()),
            ])
        );
        assert_eq!(module.components[0].state[0].id, "home#count");
        assert_eq!(module.components[1].state[0].id, "settings#count");
    }

    #[test]
    fn folder_descriptor_and_declared_screen_edits_invalidate_the_code_hash() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::remove_file(root.join("app.studio")).unwrap();
        std::fs::create_dir(root.join("screens")).unwrap();
        std::fs::write(root.join("screens/home.studio"), "<Text>Home</Text>").unwrap();
        std::fs::write(
            root.join("screens/settings.studio"),
            "<Text>Settings</Text>",
        )
        .unwrap();
        let declaration = root.join("studio.project.json");
        std::fs::write(
            &declaration,
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"screens/home.studio"},{"id":"settings","source":"screens/settings.studio"}]}"#,
        )
        .unwrap();
        let initial = compiler.inputs().unwrap();
        assert!(initial.files.contains_key(&declaration));
        assert!(matches!(
            compiler.compile(&initial).unwrap().code,
            Code::Ir(_)
        ));
        assert!(!root.join("build/dev.studio").exists());

        std::fs::write(root.join("screens/home.studio"), "<Text>Edited home</Text>").unwrap();
        let source_edit = compiler.inputs().unwrap();
        assert_ne!(initial.code_hash, source_edit.code_hash);

        std::fs::write(
            &declaration,
            r#"{"schemaVersion":1,"screens":[{"id":"settings","source":"screens/settings.studio"},{"id":"home","source":"screens/home.studio"}]}"#,
        )
        .unwrap();
        let reordered = compiler.inputs().unwrap();
        assert_ne!(source_edit.code_hash, reordered.code_hash);

        std::fs::write(root.join("screens/profile.studio"), "<Text>Profile</Text>").unwrap();
        std::fs::write(
            &declaration,
            r#"{"schemaVersion":1,"screens":[{"id":"settings","source":"screens/settings.studio"},{"id":"home","source":"screens/home.studio"},{"id":"profile","source":"screens/profile.studio"}]}"#,
        )
        .unwrap();
        let added = compiler.inputs().unwrap();
        assert_ne!(reordered.code_hash, added.code_hash);

        std::fs::remove_file(root.join("screens/profile.studio")).unwrap();
        std::fs::write(
            &declaration,
            r#"{"schemaVersion":1,"screens":[{"id":"settings","source":"screens/settings.studio"},{"id":"home","source":"screens/home.studio"}]}"#,
        )
        .unwrap();
        assert_ne!(added.code_hash, compiler.inputs().unwrap().code_hash);
    }

    #[test]
    fn external_relative_import_edits_and_removals_invalidate_the_artifact() {
        let (directory, compiler) = project();
        let outside = tempfile::tempdir().unwrap();
        let dependency = outside.path().join("shared.ts");
        std::fs::write(&dependency, "export const value = 1;").unwrap();
        let specifier = relative_import(directory.path(), &dependency).unwrap();
        std::fs::write(
            directory.path().join("helper.ts"),
            format!("export {{ value }} from {specifier:?};"),
        )
        .unwrap();
        let first = compiler.inputs().unwrap();
        assert!(first
            .files
            .contains_key(&dependency.canonicalize().unwrap()));
        std::fs::write(&dependency, "export const value = 2;").unwrap();
        let changed = compiler.inputs().unwrap();
        assert_ne!(first.code_hash, changed.code_hash);
        std::fs::remove_file(dependency).unwrap();
        assert_ne!(changed.code_hash, compiler.inputs().unwrap().code_hash);
    }

    #[test]
    fn compiled_sdk_hot_state_survives_a_real_wasm_code_replacement() {
        use studio_components::InputAction;
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::remove_file(root.join("app.studio")).unwrap();
        std::fs::create_dir(root.join("assembly")).unwrap();
        let sdk = build::repo_root().join("sdk/assemblyscript/assembly/dev.ts");
        let import = relative_import(&root.join("assembly"), &sdk).unwrap();
        let source = format!("import {{ registerHotState }} from {import:?};\n{}\nregisterHotState(\"counter\", () => count.toString(), (document: string): void => {{ count = <i32>parseInt(document); }});\n",
            include_str!("../../../../examples/starter/assembly/index.ts"));
        std::fs::write(root.join("assembly/index.ts"), &source).unwrap();
        std::fs::write(
            root.join("asconfig.json"),
            include_bytes!("../../../../examples/starter/asconfig.json"),
        )
        .unwrap();
        let artifact = compiler.compile(&compiler.inputs().unwrap()).unwrap();
        let Code::Wasm(module) = artifact.code else {
            panic!("expected Wasm");
        };
        let host = studio_runtime::StudioHost::new(
            studio_runtime::host::HostConfig::new(studio_package::TrustStore::default()),
            studio_runtime::host::NativeDisplayAvailability::Available,
        );
        let mut live = host
            .prepare_development(&artifact.manifest, (*module).clone(), artifact.assets)
            .unwrap()
            .mount_development(None)
            .unwrap();
        live.process_input("increment", InputAction::PointerClick)
            .unwrap();
        let snapshot = live.snapshot_development_state().unwrap().unwrap();
        assert_eq!(snapshot, b"studio-hot/1\n7:counter1:1");
        std::fs::write(
            root.join("assembly/index.ts"),
            source.replace("count * 125", "count * 250"),
        )
        .unwrap();
        let changed = compiler.compile(&compiler.inputs().unwrap()).unwrap();
        let Code::Wasm(next) = changed.code else {
            panic!("expected Wasm");
        };
        assert_ne!(module, next);
        let mut candidate = host
            .prepare_development(&changed.manifest, (*next).clone(), changed.assets)
            .unwrap()
            .mount_development(Some(&snapshot))
            .unwrap();
        candidate
            .process_input("increment", InputAction::KeyboardActivate)
            .unwrap();
        assert!(candidate
            .property("total", "text")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Count: 2"));
        assert!(candidate
            .property("total", "text")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("$5.00"));
        assert_eq!(
            live.snapshot_development_state().unwrap().unwrap(),
            b"studio-hot/1\n7:counter1:1",
            "candidate never mutates its predecessor"
        );
        assert!(!root.join("build/dev.studio").exists());
        // The same source builds through production without injected development exports.
        let bundle = build::build_example(&compiler.request).unwrap();
        let mut production_artifact = host
            .prepare(studio_runtime::cli::LaunchRequest::new(
                studio_runtime::cli::LaunchMode::Development,
                bundle,
            ))
            .unwrap();
        assert!(production_artifact
            .snapshot_development_state()
            .unwrap()
            .is_none());
    }

    #[test]
    fn wasm_development_compiles_loose_module_with_sdk_hooks_and_reuses_disk_cache() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::remove_file(root.join("app.studio")).unwrap();
        std::fs::create_dir(root.join("assembly")).unwrap();
        std::fs::write(
            root.join("assembly/index.ts"),
            include_bytes!("../../../../examples/starter/assembly/index.ts"),
        )
        .unwrap();
        std::fs::write(
            root.join("asconfig.json"),
            include_bytes!("../../../../examples/starter/asconfig.json"),
        )
        .unwrap();
        let inputs = compiler.inputs().unwrap();
        let artifact = compiler.compile(&inputs).unwrap();
        let Code::Wasm(module) = artifact.code else {
            panic!("expected compiled Wasm");
        };
        let host = studio_runtime::StudioHost::new(
            studio_runtime::host::HostConfig::new(studio_package::TrustStore::default()),
            studio_runtime::host::NativeDisplayAvailability::Available,
        );
        let mut surface = host
            .prepare_development(&artifact.manifest, (*module).clone(), artifact.assets)
            .unwrap()
            .mount_development(None)
            .unwrap();
        assert!(surface.snapshot_development_state().unwrap().is_none());
        assert!(!root.join("build/dev.studio").exists());
        let mut second_process = DevCompiler::new(compiler.request.clone());
        let second = second_process
            .compile(&second_process.inputs().unwrap())
            .unwrap();
        let Code::Wasm(cached) = second.code else {
            panic!("expected cache");
        };
        assert_eq!(module, cached);
        assert_eq!(
            compiler.inputs().unwrap(),
            inputs,
            "generated output cannot trigger a trailing compile"
        );
    }
    #[test]
    fn cancelling_development_compilation_reaps_the_compiler_and_discards_partial_output() {
        let (directory, mut compiler) = project();
        let root = directory.path();
        std::fs::remove_file(root.join("app.studio")).unwrap();
        std::fs::create_dir(root.join("assembly")).unwrap();
        std::fs::write(
            root.join("assembly/index.ts"),
            include_bytes!("../../../../examples/starter/assembly/index.ts"),
        )
        .unwrap();
        std::fs::write(
            root.join("asconfig.json"),
            include_bytes!("../../../../examples/starter/asconfig.json"),
        )
        .unwrap();
        compiler.set_cancellation(Arc::new(AtomicBool::new(true)));
        let inputs = compiler.inputs().unwrap();
        let Err(error) = compiler.compile(&inputs) else {
            panic!("expected cancelled compilation");
        };
        assert!(error
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cancelled")));
        assert!(!root
            .join("build/.studio-dev/cache")
            .join(format!("{}.wasm", inputs.code_hash))
            .exists());
    }

    #[test]
    fn standalone_sources_reject_reserved_generated_metadata_assets() {
        for key in [
            studio_navigation::project::ROUTING_ASSET,
            crate::studio_project::SOURCE_MAP_ASSET,
            studio_navigation::timers::TIMER_ASSET,
        ] {
            let (directory, mut compiler) = project();
            let path = directory.path().join(key);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "authored metadata must not be overwritten").unwrap();
            let inputs = compiler.inputs().unwrap();
            let error = compiler
                .compile(&inputs)
                .err()
                .expect("standalone metadata collision must fail");
            assert!(
                error
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("reserved")),
                "{key}"
            );
        }
    }
}
