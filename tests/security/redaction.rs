#![allow(missing_docs)]

use studio_app::SafeDiagnostic;
use studio_security::{ArtifactKind, OpaqueHandle, SensitiveValueFilter};
use studio_wasm::GuestDiagnostic;

const RAW_PIN: &str = "pin-8642";

#[test]
fn secrets_and_active_references_are_redacted_from_every_observable_artifact() {
    let handle = OpaqueHandle::from_bytes([0xab; 32]);
    let token = handle.to_token();
    let mut filter = SensitiveValueFilter::new();
    filter.register_secret(RAW_PIN.as_bytes()).unwrap();
    filter.register_handle(&handle);
    let injected = format!("failure secret={RAW_PIN} authorization_ref={token}");

    let artifacts = [
        GuestDiagnostic::capture(&filter, &injected)
            .message()
            .to_owned(),
        SafeDiagnostic::capture(&filter, "plugin_error", &injected)
            .message()
            .to_owned(),
        filter.sanitize_artifact(ArtifactKind::Snapshot, &injected),
        filter.sanitize_artifact(ArtifactKind::ActionResult, &injected),
        filter.sanitize_artifact(ArtifactKind::Receipt, &injected),
    ];
    for artifact in artifacts {
        assert!(artifact.contains("[REDACTED]"));
        assert!(!artifact.contains(RAW_PIN));
        assert!(!artifact.contains(&token));
    }
}

#[test]
fn persistence_rejects_sensitive_values_instead_of_writing_redacted_ambiguity() {
    let handle = OpaqueHandle::from_bytes([0xcd; 32]);
    let token = handle.to_token();
    let mut filter = SensitiveValueFilter::new();
    filter.register_secret(RAW_PIN.as_bytes()).unwrap();
    filter.register_handle(&handle);

    assert!(filter.validate_persistence("ordinary receipt text").is_ok());
    assert!(
        filter
            .validate_persistence(&format!("PIN={RAW_PIN}"))
            .is_err()
    );
    assert!(
        filter
            .validate_persistence(&format!("handle={token}"))
            .is_err()
    );
    assert!(!format!("{filter:?}").contains(RAW_PIN));
    assert!(!format!("{filter:?}").contains(&token));
}
