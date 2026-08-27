//! Deterministic contract tests for ticket 56 settings and support surfaces.

use std::sync::{Arc, Mutex};

use studio_app::SafeDiagnostic;
use studio_app::{
    AboutInfo, AccessibilitySettings, BuildSigningSettings, DiagnosticConsent, FeedbackConsent,
    FeedbackDraft, FeedbackError, FeedbackPayload, FeedbackService, FeedbackTransport,
    GlobalSettingChange, HelpNavigator, InMemorySettingsPersistence, KeyboardAction,
    LanguagePreference, NavigationResult, ProjectSettingChange, RuntimeIdentitySettings,
    SettingsController, SettingsEffect, StorageMode, StorageSyncSettings, SupportNavigator,
    SupportSurface, ThemePreference, UpdateChannel,
};
use studio_security::SensitiveValueFilter;

#[test]
fn global_preferences_persist_and_publish_immediate_effects() {
    let store = InMemorySettingsPersistence::default();
    let mut controller = SettingsController::new(store.clone()).expect("defaults load");

    let effect = controller
        .update_global(GlobalSettingChange::Theme(ThemePreference::Dark))
        .expect("theme persists");
    assert_eq!(effect, SettingsEffect::ThemeChanged(ThemePreference::Dark));
    controller
        .update_global(GlobalSettingChange::Language(LanguagePreference::German))
        .expect("language persists");
    controller
        .update_global(GlobalSettingChange::ReducedMotion(true))
        .expect("motion persists");
    assert_eq!(controller.global().theme, ThemePreference::Dark);
    assert_eq!(controller.global().language, LanguagePreference::German);
    assert!(controller.global().reduced_motion);
    assert_eq!(controller.take_effects().len(), 3);

    let reopened = SettingsController::new(store).expect("reopen settings");
    assert_eq!(reopened.global().theme, ThemePreference::Dark);
    assert_eq!(reopened.global().language, LanguagePreference::German);
    assert!(reopened.global().reduced_motion);
}

#[test]
fn project_settings_keep_identity_opaque_and_persist_categories() {
    let store = InMemorySettingsPersistence::default();
    let mut controller = SettingsController::new(store.clone()).expect("defaults load");
    controller
        .update_project(
            "project-1",
            ProjectSettingChange::StorageSync(StorageSyncSettings {
                mode: StorageMode::Cloud,
                sync_enabled: true,
                keep_local_copy: true,
            }),
        )
        .expect("storage settings persist");
    controller
        .update_project(
            "project-1",
            ProjectSettingChange::RuntimeIdentity(RuntimeIdentitySettings {
                identity_ref: Some("host-identity-ref".to_owned()),
                display_name: Some("Operations".to_owned()),
            }),
        )
        .expect("identity selection persists");
    controller
        .update_project(
            "project-1",
            ProjectSettingChange::BuildSigning(BuildSigningSettings {
                signing_profile: Some("release".to_owned()),
                require_signature: true,
                channel: UpdateChannel::Preview,
            }),
        )
        .expect("build settings persist");

    let project = controller.project("project-1").expect("project loads");
    assert_eq!(project.storage_sync.mode, StorageMode::Cloud);
    assert!(project.storage_sync.sync_enabled);
    assert_eq!(
        project.runtime_identity.identity_ref.as_deref(),
        Some("host-identity-ref")
    );
    assert_eq!(project.build_signing.channel, UpdateChannel::Preview);
    assert_eq!(store.project("project-1"), Some(project));
}

#[derive(Clone, Default)]
struct RecordingTransport {
    sent: Arc<Mutex<Vec<FeedbackPayload>>>,
}

impl FeedbackTransport for RecordingTransport {
    fn send(&mut self, payload: &FeedbackPayload) -> Result<(), FeedbackError> {
        self.sent
            .lock()
            .expect("recording lock")
            .push(payload.clone());
        Ok(())
    }
}

#[test]
fn feedback_requires_send_consent_and_diagnostic_consent() {
    let transport = RecordingTransport::default();
    let sent = transport.sent.clone();
    let filter = SensitiveValueFilter::new();
    let diagnostic = SafeDiagnostic::capture(&filter, "runtime_error", "token=never-send-this");
    let mut service = FeedbackService::new(transport, DiagnosticConsent::Denied, filter);
    let draft = FeedbackDraft {
        message: "The preview is confusing".to_owned(),
        include_diagnostics: true,
        consent: FeedbackConsent::NotGranted,
    };
    assert_eq!(
        service.submit(&draft, std::slice::from_ref(&diagnostic)),
        Err(FeedbackError::ConsentRequired)
    );
    assert!(sent.lock().expect("recording lock").is_empty());

    let granted = FeedbackDraft {
        consent: FeedbackConsent::Granted,
        ..draft
    };
    service
        .submit(&granted, std::slice::from_ref(&diagnostic))
        .expect("message sends");
    let payload = sent
        .lock()
        .expect("recording lock")
        .pop()
        .expect("one send");
    assert!(
        payload.diagnostics.is_empty(),
        "diagnostic consent was denied"
    );

    service.set_diagnostic_consent(DiagnosticConsent::Granted);
    service
        .submit(&granted, std::slice::from_ref(&diagnostic))
        .expect("consented diagnostics send");
    let payload = sent
        .lock()
        .expect("recording lock")
        .pop()
        .expect("one send");
    assert_eq!(payload.diagnostics.len(), 1);
    assert!(!payload.diagnostics[0].message.contains("never-send-this"));
}

#[test]
fn support_and_help_surfaces_have_stable_keyboard_order() {
    let mut nav = SupportNavigator::new();
    assert_eq!(nav.current(), SupportSurface::Settings);
    assert_eq!(
        nav.handle(KeyboardAction::Tab),
        NavigationResult::Focused(SupportSurface::Help)
    );
    assert_eq!(
        nav.handle(KeyboardAction::ArrowDown),
        NavigationResult::Focused(SupportSurface::Feedback)
    );
    assert_eq!(
        nav.handle(KeyboardAction::ShiftTab),
        NavigationResult::Focused(SupportSurface::Help)
    );
    assert_eq!(
        nav.handle(KeyboardAction::Enter),
        NavigationResult::Activated(SupportSurface::Help)
    );
    assert_eq!(
        nav.handle(KeyboardAction::Escape),
        NavigationResult::Dismissed
    );

    let mut help = HelpNavigator::new();
    assert_eq!(help.current().title, "Welcome to Studio");
    help.next();
    assert_eq!(help.current().topic, studio_app::HelpTopic::Identity);
    help.previous();
    assert_eq!(help.current().steps.len(), 3);

    let about = AboutInfo::studio("0.1.0", UpdateChannel::Stable);
    assert_eq!(about.product_name, "Studio Designer");
    assert_eq!(about.version, "0.1.0");
    assert!(!about.notices.is_empty());
}

#[test]
fn accessibility_update_is_typed_and_immediate() {
    let store = InMemorySettingsPersistence::default();
    let mut controller = SettingsController::new(store).expect("defaults load");
    let settings = AccessibilitySettings {
        large_text: true,
        high_contrast: true,
        descriptive_labels: true,
    };
    assert_eq!(
        controller
            .update_global(GlobalSettingChange::Accessibility(settings.clone()))
            .expect("accessibility persists"),
        SettingsEffect::AccessibilityChanged(settings.clone())
    );
    assert_eq!(controller.global().accessibility, settings);
}
