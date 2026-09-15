#![allow(missing_docs)]

use studio_host::{
    AssetAdmission, AssetProvenance, AssetUsage, DeletePolicy, Durability, LibraryAssetStore,
    LibraryDiagnosticCode, LibraryPanelAction, LibraryPanelKey, LibraryPanelState,
    RuntimeVariantSpec,
};
use tokio::runtime::Builder;

fn file(source: &str, bytes: &[u8]) -> AssetAdmission {
    AssetAdmission::new(
        "photo.png",
        bytes.to_vec(),
        AssetProvenance::new(source, "designer"),
    )
}

#[test]
fn identical_sources_deduplicate_and_preserve_original_and_variant() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let library = LibraryAssetStore::open(directory.path(), Durability::Every)
                .await
                .expect("library opens");
            let bytes = b"\x89PNG\r\n\x1a\nsource";
            let first = library
                .admit(file("file-a", bytes))
                .await
                .expect("first admission");
            let second = library
                .admit(file("file-b", bytes))
                .await
                .expect("deduplicated admission");
            assert_eq!(first.id, second.id);
            assert_eq!(first.content_hash, second.content_hash);
            assert_eq!(library.list().await.expect("list").len(), 1);
            assert_eq!(
                library
                    .read_original(&first.id)
                    .await
                    .expect("original")
                    .bytes,
                bytes
            );
            assert_eq!(second.provenance.len(), 2);

            let spec = RuntimeVariantSpec {
                width: Some(320),
                height: Some(240),
                ..RuntimeVariantSpec::default()
            };
            let variant_a = library
                .generate_runtime_variant(&first.id, spec.clone())
                .await
                .expect("variant");
            let variant_b = library
                .generate_runtime_variant(&first.id, spec.clone())
                .await
                .expect("same variant");
            assert_eq!(variant_a, variant_b);
            assert_eq!(
                library
                    .read_runtime_variant(&first.id, &spec)
                    .await
                    .expect("variant bytes")
                    .bytes,
                bytes
            );
            library.close().await.expect("library closes");

            let reopened = LibraryAssetStore::open(directory.path(), Durability::Every)
                .await
                .expect("library reopens");
            let persisted = reopened.list().await.expect("persisted catalog");
            assert_eq!(persisted.len(), 1);
            assert_eq!(persisted[0].id, first.id);
            assert_eq!(
                reopened
                    .read_original(&first.id)
                    .await
                    .expect("persisted original")
                    .bytes,
                bytes
            );
        });
}

#[test]
fn unsafe_svg_and_unsupported_codec_are_diagnosed_at_admission() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let library = LibraryAssetStore::open(directory.path(), Durability::Every)
                .await
                .expect("library opens");
            let svg = AssetAdmission::new(
                "icon.svg",
                br"<svg><script>alert(1)</script></svg>".to_vec(),
                AssetProvenance::new("upload", "designer"),
            );
            let svg_error = library.admit(svg).await.expect_err("unsafe SVG rejected");
            assert_eq!(
                svg_error.diagnostic().code(),
                LibraryDiagnosticCode::UnsafeSvg
            );
            assert!(svg_error.diagnostic().message().contains("script"));

            let video = AssetAdmission::new(
                "clip.mp4",
                b"....ftypisom....codec=vp6".to_vec(),
                AssetProvenance::new("upload", "designer"),
            );
            let codec_error = library
                .admit(video)
                .await
                .expect_err("unsupported codec rejected");
            assert_eq!(
                codec_error.diagnostic().code(),
                LibraryDiagnosticCode::UnsupportedCodec
            );
            assert!(codec_error.diagnostic().message().contains("codec"));
        });
}

#[test]
fn deletion_reports_usage_and_panel_traversal_is_keyboard_safe() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let library = LibraryAssetStore::open(directory.path(), Durability::Every)
                .await
                .expect("library opens");
            let asset = library
                .admit(file("file-a", b"\x89PNG\r\n\x1a\nasset"))
                .await
                .expect("admit");
            let usage = AssetUsage::new("node-1", "node-1", "image").expect("usage");
            library.bind(&asset.id, usage.clone()).await.expect("bind");
            let error = library
                .delete(&asset.id)
                .await
                .expect_err("referenced asset is protected");
            assert_eq!(error.diagnostic().code(), LibraryDiagnosticCode::AssetInUse);
            assert_eq!(error.usages(), std::slice::from_ref(&usage));
            let deleted = library
                .delete_with_policy(&asset.id, DeletePolicy::AllowBreakingChange)
                .await
                .expect("explicit breaking deletion");
            assert_eq!(deleted.broken_references, vec![usage]);

            let second = library
                .admit(file("file-b", b"\x89PNG\r\n\x1a\nsecond"))
                .await
                .expect("second asset");
            let listed = library.list().await.expect("list for panel");
            let mut panel = LibraryPanelState::new(&listed);
            assert_eq!(panel.focus_order().len(), 1);
            assert_eq!(panel.focused_asset(), Some(&second.id));
            assert_eq!(
                panel.handle_key(LibraryPanelKey::Enter),
                LibraryPanelAction::Activated(Some(second.id.clone()))
            );
            assert_eq!(
                panel.handle_key(LibraryPanelKey::Down),
                LibraryPanelAction::Focused(Some(second.id))
            );
            assert_eq!(
                panel.handle_key(LibraryPanelKey::Escape),
                LibraryPanelAction::Cancelled
            );
        });
}
