use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Dev { example, port } => dev(&example, port),
        Commands::Build { example } => build(&example),
        Commands::Preview { example } => preview(&example),
        Commands::Generate => generate(),
    }
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
