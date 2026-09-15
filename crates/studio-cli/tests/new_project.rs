//! `studio new` contract: gallery scaffolding, template variants, usage
//! errors, and remote acquisition through a loopback template server.

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Output;
use tempfile::TempDir;

fn run_new(dir: &std::path::Path, args: &[&str]) -> Output {
    std::process::Command::new(common::studio_binary())
        .args(["new"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn studio binary")
}

fn combined(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

fn assert_ok(output: &Output, context: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{context}: {}",
        combined(output)
    );
}

#[test]
fn blank_template_scaffolds_builds_and_rewrites_identity() {
    let root = TempDir::new().unwrap();
    let output = run_new(root.path(), &["demo-shop"]);
    assert_ok(&output, "new blank");
    let project = root.path().join("demo-shop");
    assert!(project.join("app.studio").is_file());
    assert!(project.join("manifest.json").is_file());
    assert!(
        !project.join("template.json").exists(),
        "gallery manifest stays out of the project"
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["id"], "com.studio.demo.shop");
    assert_eq!(manifest["name"], "Demo Shop");

    let build = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_ok(&build, "scaffolded project builds");
    assert!(project.join("build/demo-shop.studio").is_file());
}

#[test]
fn gallery_variants_check_clean() {
    let root = TempDir::new().unwrap();
    for template in ["blank", "ecommerce", "social"] {
        let name = format!("proj-{template}");
        let output = run_new(root.path(), &[&name, "-t", template]);
        assert_ok(&output, "new {template}");
        let check = common::run_studio(&[
            "check",
            root.path().join(&name).join("app.studio").to_str().unwrap(),
        ]);
        assert_ok(&check, "{template} checks clean");
    }
}

#[test]
fn usage_errors_are_stable_and_safe() {
    let root = TempDir::new().unwrap();
    let bad = run_new(root.path(), &["Bad Name"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(combined(&bad).contains("BUILD_NEW_NAME_INVALID"));

    let occupied = root.path().join("taken");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("keep.txt"), "do not touch").unwrap();
    let refused = run_new(root.path(), &["taken"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(combined(&refused).contains("BUILD_NEW_DESTINATION_OCCUPIED"));

    let forced = run_new(root.path(), &["taken", "--force"]);
    assert_ok(&forced, "force overwrites");
    assert!(!occupied.join("keep.txt").exists());
    assert!(occupied.join("app.studio").is_file());

    let unknown = run_new(root.path(), &["nope", "-t", "missing"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(combined(&unknown).contains("BUILD_NEW_TEMPLATE_UNKNOWN"));

    let plain_http = run_new(root.path(), &["nope", "-t", "http://example.com/x.zip"]);
    assert_eq!(plain_http.status.code(), Some(2));
    assert!(combined(&plain_http).contains("BUILD_NEW_REMOTE_REJECTED"));
}

#[test]
fn list_templates_names_the_gallery() {
    let root = TempDir::new().unwrap();
    let output = run_new(root.path(), &["--list-templates"]);
    assert_ok(&output, "list templates");
    let text = combined(&output);
    for template in ["blank", "ecommerce", "social"] {
        assert!(text.contains(template), "lists {template}");
    }

    // Machine contract for the designer IDE.
    let output = run_new(root.path(), &["--list-templates", "--format", "json"]);
    assert_ok(&output, "list templates as json");
    let items: Vec<serde_json::Value> =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json array");
    assert_eq!(items.len(), 3);
    for item in &items {
        assert!(item.get("id").and_then(|id| id.as_str()).is_some());
        assert!(item.get("kind").and_then(|kind| kind.as_str()).is_some());
        assert!(item
            .get("description")
            .and_then(|description| description.as_str())
            .is_some());
    }
    let ids: Vec<&str> = items
        .iter()
        .filter_map(|item| item.get("id").and_then(|id| id.as_str()))
        .collect();
    assert_eq!(ids, vec!["blank", "ecommerce", "social"]);
}

/// Minimal loopback template server: one zip and one template bundle.
fn serve_templates() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let zip_bytes = template_zip();
    let bundle_bytes = template_bundle();
    let handle = std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = [0u8; 1024];
            let Ok(read) = stream.read(&mut request) else {
                return;
            };
            let request = String::from_utf8_lossy(&request[..read]);
            let body = if request.contains("GET /t.zip ") {
                zip_bytes.clone()
            } else if request.contains("GET /t.studio ") {
                bundle_bytes.clone()
            } else {
                let _ = stream.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                continue;
            };
            let _ = stream.write_all(
                format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes(),
            );
            let _ = stream.write_all(&body);
        }
    });
    (format!("http://{address}"), handle)
}

/// A template zip as codeload would serve it: single top-level directory.
fn template_zip() -> Vec<u8> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/blank");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in ["template.json", "manifest.json", "app.studio"] {
        let bytes = std::fs::read(repo.join(name)).unwrap();
        writer
            .start_file(
                format!("blank-main/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        use std::io::Write as _;
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A self-describing template bundle: manifest, entry source, gallery manifest.
fn template_bundle() -> Vec<u8> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/blank");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in ["template.json", "manifest.json", "app.studio"] {
        let bytes = std::fs::read(repo.join(name)).unwrap();
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        use std::io::Write as _;
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

use std::path::PathBuf;

#[test]
fn remote_templates_expand_and_verify() {
    let root = TempDir::new().unwrap();
    let (base, server) = serve_templates();
    for (name, url) in [
        ("remote-zip", format!("{base}/t.zip")),
        ("remote-bundle", format!("{base}/t.studio")),
    ] {
        let output = run_new(root.path(), &[name, "-t", &url]);
        assert_ok(&output, "remote {url}");
        let project = root.path().join(name);
        assert!(project.join("app.studio").is_file());
        let check = common::run_studio(&["check", project.join("app.studio").to_str().unwrap()]);
        assert_ok(&check, "remote tree checks clean");
    }
    server.join().unwrap();
}

#[test]
fn publish_template_writes_deterministic_verified_bundle() {
    let root = TempDir::new().unwrap();
    let output = run_new(root.path(), &["pub-src", "-t", "blank"]);
    assert_ok(&output, "seed source project");
    let project = root.path().join("pub-src");

    // Gallery trees skip template.json on copy; publishing needs one.
    std::fs::write(
        project.join("template.json"),
        r#"{"id":"pub-src","name":"Pub Src","description":"round trip","kind":"layout"}"#,
    )
    .unwrap();

    let first = common::run_studio(&["build", "--as-template", project.to_str().unwrap()]);
    assert_ok(&first, "publish template");
    let bundle = project.join("build/pub-src.template.studio");
    assert!(bundle.is_file());
    let first_bytes = std::fs::read(&bundle).unwrap();
    let second = common::run_studio(&["build", "--as-template", project.to_str().unwrap()]);
    assert_ok(&second, "publish again");
    assert_eq!(first_bytes, std::fs::read(&bundle).unwrap());

    // Assembly projects fail closed (seeded starter builds, but has no
    // studio entry to publish).
    let assembly = common::seed_project(root.path(), "assembly-proj");
    let built = common::run_studio(&["build", assembly.to_str().unwrap()]);
    assert_ok(&built, "assembly seed builds");
    let refused = common::run_studio(&["build", "--as-template", assembly.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(combined(&refused).contains("BUILD_TEMPLATE_ASSEMBLY_UNSUPPORTED"));

    // Missing template.json fails closed with no artifact.
    std::fs::remove_file(project.join("template.json")).unwrap();
    let missing = common::run_studio(&["build", "--as-template", project.to_str().unwrap()]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(combined(&missing).contains("BUILD_TEMPLATE_MANIFEST_MISSING"));
}

/// Serve one directory over loopback for the publish round-trip.
fn serve_dir(root: PathBuf) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0u8; 1024];
        let Ok(read) = stream.read(&mut request) else {
            return;
        };
        let request = String::from_utf8_lossy(&request[..read]);
        let path = request
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .trim_start_matches('/');
        let file = root.join(path);
        // Traversal-safe: the join must stay under the root.
        let body = if path.contains("..") {
            None
        } else {
            std::fs::read(&file).ok()
        };
        match body {
            Some(body) => {
                let _ = stream.write_all(
                    format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes(),
                );
                let _ = stream.write_all(&body);
            }
            None => {
                let _ = stream.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        }
    });
    (format!("http://{address}"), handle)
}

#[test]
fn published_bundle_scaffolds_over_loopback() {
    let root = TempDir::new().unwrap();
    let output = run_new(root.path(), &["pub-src", "-t", "blank"]);
    assert_ok(&output, "seed source project");
    let project = root.path().join("pub-src");
    std::fs::write(
        project.join("template.json"),
        r#"{"id":"pub-src","name":"Pub Src","description":"round trip","kind":"layout"}"#,
    )
    .unwrap();
    let published = common::run_studio(&["build", "--as-template", project.to_str().unwrap()]);
    assert_ok(&published, "publish template");

    let (base, server) = serve_dir(project.join("build"));
    let output = run_new(
        root.path(),
        &[
            "round-trip",
            "-t",
            &format!("{base}/pub-src.template.studio"),
        ],
    );
    assert_ok(&output, "scaffold from published bundle");
    let expanded = root.path().join("round-trip");
    assert!(expanded.join("app.studio").is_file());
    assert!(expanded.join("manifest.json").is_file());
    let check = common::run_studio(&["check", expanded.join("app.studio").to_str().unwrap()]);
    assert_ok(&check, "round-tripped tree checks clean");
    server.join().unwrap();
}
