#![allow(missing_docs)]

use ed25519_dalek::Signer;
use studio_package::{TrustStore, TrustedPublisherKey, canonical_document_bytes};
use studio_update_channel::{
    HostError, InstallationEventKind, InstallationHost, InstallationOutcome, InstallationState,
    MemoryStateStore, SignedUpdate, UpdateChannel, UpdateDocument, UpdateStateStore,
    VerifiedUpdate, artifact_digest,
};

struct Host {
    fail_health: bool,
    installs: Vec<String>,
    rollbacks: Vec<String>,
}

impl InstallationHost for Host {
    fn install(&mut self, id: &str, update: &VerifiedUpdate) -> Result<(), HostError> {
        self.installs
            .push(format!("{id}:{}", update.document().version));
        Ok(())
    }

    fn health_check(&mut self, _id: &str, _version: &str) -> Result<(), HostError> {
        if self.fail_health {
            Err(HostError::Health)
        } else {
            Ok(())
        }
    }

    fn rollback(&mut self, id: &str, version: &str) -> Result<(), HostError> {
        self.rollbacks.push(format!("{id}:{version}"));
        Ok(())
    }
}

fn baseline(store: &mut MemoryStateStore, id: &str) {
    store
        .save(
            InstallationState {
                installation_id: id.to_owned(),
                active_version: "1.0.0".to_owned(),
                previous_version: None,
                staged_update_id: None,
                last_error: None,
                revision: 0,
                history: Vec::new(),
            },
            0,
        )
        .unwrap();
}

#[test]
fn signed_candidate_is_admitted_and_tampering_is_rejected() {
    let artifact = b"update".to_vec();
    let document = UpdateDocument {
        document_version: 1,
        update_id: "release-2".to_owned(),
        version: "2.0.0".to_owned(),
        channel: "stable".to_owned(),
        artifact_sha256: artifact_digest(&artifact),
        rollout_percent: 100,
        publisher_id: "publisher".to_owned(),
        key_id: "key".to_owned(),
        migration_id: None,
    };
    let signing = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let trust = TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "publisher".to_owned(),
        key_id: "key".to_owned(),
        verifying_key: signing.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap();
    let signed = SignedUpdate {
        signature: ed25519_dalek::Signer::sign(
            &signing,
            &canonical_document_bytes(&serde_json::to_value(&document).unwrap()).unwrap(),
        )
        .to_bytes()
        .to_vec(),
        document,
        artifact,
    };
    assert!(VerifiedUpdate::admit(signed.clone(), &trust).is_ok());
    let mut tampered = signed;
    tampered.artifact.push(0);
    assert!(VerifiedUpdate::admit(tampered, &trust).is_err());
}

#[test]
fn rollout_activates_and_health_failure_rolls_back() {
    let artifact = b"update".to_vec();
    let document = UpdateDocument {
        document_version: 1,
        update_id: "release-2".to_owned(),
        version: "2.0.0".to_owned(),
        channel: "stable".to_owned(),
        artifact_sha256: artifact_digest(&artifact),
        rollout_percent: 100,
        publisher_id: "publisher".to_owned(),
        key_id: "key".to_owned(),
        migration_id: None,
    };
    let signing = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let trust = TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "publisher".to_owned(),
        key_id: "key".to_owned(),
        verifying_key: signing.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap();
    let value = serde_json::to_value(&document).unwrap();
    let signature =
        ed25519_dalek::Signer::sign(&signing, &canonical_document_bytes(&value).unwrap())
            .to_bytes()
            .to_vec();
    let update = VerifiedUpdate::admit(
        SignedUpdate {
            document,
            signature,
            artifact,
        },
        &trust,
    )
    .unwrap();
    let mut store = MemoryStateStore::default();
    baseline(&mut store, "device-a");
    let mut channel = UpdateChannel::new(store, "stable").unwrap();
    let mut host = Host {
        fail_health: false,
        installs: Vec::new(),
        rollbacks: Vec::new(),
    };
    let report = channel
        .roll_out(&update, &["device-a".to_owned()], &mut host)
        .unwrap();
    assert_eq!(report.outcomes, vec![InstallationOutcome::Activated]);
    assert_eq!(
        channel.state_store().states()["device-a"].active_version,
        "2.0.0"
    );

    let next_document = UpdateDocument {
        update_id: "release-3".to_owned(),
        version: "3.0.0".to_owned(),
        ..update.document().clone()
    };
    let next_value = serde_json::to_value(&next_document).unwrap();
    let next_signature = signing
        .sign(&canonical_document_bytes(&next_value).unwrap())
        .to_bytes()
        .to_vec();
    let next = VerifiedUpdate::admit(
        SignedUpdate {
            document: next_document,
            signature: next_signature,
            artifact: b"update".to_vec(),
        },
        &trust,
    )
    .unwrap();
    let mut failing = UpdateChannel::new(channel.state_store().clone(), "stable").unwrap();
    host.fail_health = true;
    let report = failing
        .roll_out(&next, &["device-a".to_owned()], &mut host)
        .unwrap();
    assert_eq!(
        report.outcomes,
        vec![InstallationOutcome::Failed(
            studio_update_channel::UpdateErrorCode::HealthCheck
        )]
    );
    assert_eq!(
        failing.state_store().states()["device-a"].active_version,
        "2.0.0"
    );
    assert!(
        failing.state_store().states()["device-a"]
            .history
            .iter()
            .any(|event| event.kind == InstallationEventKind::RolledBack)
    );
}
