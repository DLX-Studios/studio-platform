#![allow(missing_docs)]

use std::{fs, path::PathBuf};

use studio_security::{ProtectedSecretErrorCode, SecretInput};

const RELEASE_CANARY: &[u8] = b"studio-default-credential-do-not-ship";

#[test]
fn known_default_credentials_fail_protected_authentication_admission() {
    for known_default in [
        b"password".as_slice(),
        b"admin".as_slice(),
        b"changeme".as_slice(),
        b"123456".as_slice(),
        b"default".as_slice(),
        RELEASE_CANARY,
    ] {
        assert_eq!(
            SecretInput::new(known_default.to_vec()).unwrap_err().code(),
            ProtectedSecretErrorCode::CredentialRejected
        );
    }
}

#[test]
fn compiled_production_library_contains_no_release_default_credential() {
    let dependency_directory = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let artifacts = production_library_artifacts(&dependency_directory);
    assert!(
        !artifacts.is_empty(),
        "Cargo should produce a studio-security library artifact"
    );
    for artifact in artifacts {
        let bytes = fs::read(&artifact).unwrap();
        assert!(
            !contains(&bytes, RELEASE_CANARY),
            "release default credential found in {}",
            artifact.display()
        );
    }
}

fn production_library_artifacts(directory: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let is_security_library = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("libstudio_security-"));
            let is_library_artifact = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("rlib")
                        || extension.eq_ignore_ascii_case("rmeta")
                });
            is_security_library && is_library_artifact
        })
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
