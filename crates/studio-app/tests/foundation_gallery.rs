//! Deterministic contract tests for the native foundation gallery.

use std::collections::BTreeSet;

use studio_app::foundation::{AnimationPolicy, FoundationFeature, FoundationGalleryModel};

#[test]
fn gallery_model_exposes_the_native_foundation_contract() {
    let mut gallery = FoundationGalleryModel::new(false);

    assert_eq!(
        gallery.features(),
        BTreeSet::from([
            FoundationFeature::AccessibleLabels,
            FoundationFeature::Animation,
            FoundationFeature::Button,
            FoundationFeature::FocusTraversal,
            FoundationFeature::Popup,
            FoundationFeature::Scroll,
            FoundationFeature::Text,
            FoundationFeature::TextInput,
        ])
    );
    assert_eq!(
        gallery.accessibility_labels(),
        ["Increment counter", "Operator note", "Open details"]
    );
    assert_eq!(gallery.animation_policy(), AnimationPolicy::HostScheduled);

    gallery.activate_button();
    gallery.replace_text("ready");
    gallery.scroll_by(24.0);
    gallery.toggle_popup();
    gallery.focus_next();

    let snapshot = gallery.snapshot();
    assert_eq!(snapshot.button_activations, 1);
    assert_eq!(snapshot.text_input, "ready");
    assert!((snapshot.scroll_offset - 24.0).abs() < f32::EPSILON);
    assert!(snapshot.popup_open);
    assert_eq!(snapshot.focused_label, Some("Operator note"));
}

#[test]
fn reduced_motion_preserves_state_with_a_static_animation_policy() {
    let mut gallery = FoundationGalleryModel::new(true);

    assert!(gallery.reduced_motion());
    assert_eq!(gallery.animation_policy(), AnimationPolicy::Static);

    gallery.activate_button();
    gallery.replace_text("ready");

    let snapshot = gallery.snapshot();
    assert_eq!(snapshot.button_activations, 1);
    assert_eq!(snapshot.text_input, "ready");
}
