//! Sprint-04 surface checks: T49 content editors, T51 conversation composer,
//! and T53 plugin/template UX driven through the real `DesignerSession`
//! command authority.

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, CollectionId, ContentCollection, ContentCollectionSchema,
    ContentFieldKind, ContentFieldSchema, ContentRecord, DefaultDesignerSession, DesignerSession,
    FormDefinition, FormFieldSchema, FormId, InMemoryDesignerPersistence, OperationId, PluginId,
    ProjectId, PropertyValue, RecordId, STUDIO_DESIGN_SCHEMA_VERSION, SettingKey, SettingValue,
    SourceProvenance, StudioDesign, UndoGroupId,
};
use studio_designer::{
    ContentEditor, ContentEditorError, ConversationComposer, PluginSurfaceError, PluginTemplateUx,
};
use studio_plugin_registry::{
    CompatibilityRange, Contributions, DescriptorPublisher, PluginDescriptorV1, SettingsField,
    SettingsFieldType, SettingsGroup,
};

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("sprint04-human").unwrap(),
        kind: ActorKind::Human,
        display_name: "Sprint 04 Human".to_owned(),
    }
}

fn editor_session() -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    block_on(DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        StudioDesign::empty(ProjectId::new("sprint04-project").unwrap(), "Sprint 04"),
        OperationId::new("sprint04-create").unwrap(),
        actor(),
        UndoGroupId::new("sprint04-create").unwrap(),
    ))
    .unwrap()
}

fn menu_collection() -> ContentCollection {
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::String,
            required: true,
        },
    );
    ContentCollection {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: CollectionId::new("menu-items").unwrap(),
        name: "Menu items".to_owned(),
        schema: ContentCollectionSchema {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            fields,
        },
        records: BTreeMap::new(),
        fixture: studio_design::ContentFixture::default(),
    }
}

fn menu_form() -> FormDefinition {
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        FormFieldSchema {
            kind: ContentFieldKind::String,
            required: true,
            minimum_length: Some(2),
            maximum_length: None,
            pattern: None,
            minimum_value: None,
            maximum_value: None,
        },
    );
    FormDefinition {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: FormId::new("menu-item-form").unwrap(),
        name: "Menu item form".to_owned(),
        fields,
        target_collection_id: Some(CollectionId::new("menu-items").unwrap()),
    }
}

fn form_session() -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    let mut design = StudioDesign::empty(ProjectId::new("sprint04-forms").unwrap(), "Sprint forms");
    design
        .forms
        .insert(FormId::new("menu-item-form").unwrap(), menu_form());
    block_on(DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        design,
        OperationId::new("sprint04-forms-create").unwrap(),
        actor(),
        UndoGroupId::new("sprint04-forms-create").unwrap(),
    ))
    .unwrap()
}

fn descriptor() -> PluginDescriptorV1 {
    PluginDescriptorV1 {
        schema_version: 1,
        id: "com.dlx.postools".to_owned(),
        name: "POS Tools".to_owned(),
        version: "1.0.0".to_owned(),
        publisher: DescriptorPublisher {
            id: "dlx".to_owned(),
            key_id: "dlx-2026".to_owned(),
        },
        compatibility: CompatibilityRange {
            studio_version: "^0.1.0".to_owned(),
            schema_versions: vec![1],
        },
        contributions: Contributions {
            settings_groups: vec![
                SettingsGroup {
                    id: "general".to_owned(),
                    title: "General".to_owned(),
                    fields: vec![SettingsField {
                        id: "printer-name".to_owned(),
                        label: "Printer name".to_owned(),
                        kind: SettingsFieldType::Text {
                            default: None,
                            max_length: Some(16),
                        },
                    }],
                },
                SettingsGroup {
                    id: "printer".to_owned(),
                    title: "Printer".to_owned(),
                    fields: vec![SettingsField {
                        id: "receipt-copies".to_owned(),
                        label: "Receipt copies".to_owned(),
                        kind: SettingsFieldType::Number {
                            min: Some(0.0),
                            max: Some(5.0),
                            default: Some(1.0),
                        },
                    }],
                },
            ],
            ..Contributions::default()
        },
        capabilities: Vec::new(),
        lifecycle: Vec::new(),
    }
}

fn printer_key() -> SettingKey {
    SettingKey {
        plugin_id: PluginId::new("com.dlx.postools").unwrap(),
        group_id: "general".to_owned(),
        field_id: "printer-name".to_owned(),
    }
}

fn copies_key() -> SettingKey {
    SettingKey {
        plugin_id: PluginId::new("com.dlx.postools").unwrap(),
        group_id: "printer".to_owned(),
        field_id: "receipt-copies".to_owned(),
    }
}

#[test]
fn t49_collections_flow_through_the_session_authority() {
    let mut session = editor_session();
    let editor = ContentEditor::new();

    let receipt = block_on(editor.create_collection(
        &mut session,
        menu_collection(),
        OperationId::new("t49-create").unwrap(),
        actor(),
        UndoGroupId::new("t49-create").unwrap(),
    ))
    .unwrap();
    assert_eq!(receipt.committed_revision.get(), 1);

    let collections = editor.collections(&session);
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "Menu items");

    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::String,
            required: true,
        },
    );
    fields.insert(
        "price".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::Decimal,
            required: false,
        },
    );
    let receipt = block_on(editor.update_collection_schema(
        &mut session,
        CollectionId::new("menu-items").unwrap(),
        ContentCollectionSchema {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            fields,
        },
        OperationId::new("t49-schema").unwrap(),
        actor(),
        UndoGroupId::new("t49-schema").unwrap(),
    ))
    .unwrap();
    assert_eq!(receipt.committed_revision.get(), 2);

    let updated = editor
        .collection(&session, &CollectionId::new("menu-items").unwrap())
        .unwrap();
    assert!(updated.schema.fields.contains_key("price"));
}

#[test]
fn t49_collection_failures_are_safe_and_structured() {
    let mut session = editor_session();
    let editor = ContentEditor::new();

    block_on(editor.create_collection(
        &mut session,
        menu_collection(),
        OperationId::new("t49-first").unwrap(),
        actor(),
        UndoGroupId::new("t49-first").unwrap(),
    ))
    .unwrap();

    let duplicate = block_on(editor.create_collection(
        &mut session,
        menu_collection(),
        OperationId::new("t49-second").unwrap(),
        actor(),
        UndoGroupId::new("t49-second").unwrap(),
    ))
    .unwrap_err();
    let ContentEditorError::Rejected(diagnostics) = duplicate else {
        panic!("expected a structured session rejection");
    };
    assert_eq!(diagnostics[0].code, "CONTENT_COLLECTION_EXISTS");

    assert!(matches!(
        editor.collection(&session, &CollectionId::new("missing").unwrap()),
        Err(ContentEditorError::CollectionMissing(_))
    ));
}

#[test]
fn t49_form_validation_never_mutates_the_session() {
    let session = form_session();
    let editor = ContentEditor::new();
    let form_id = FormId::new("menu-item-form").unwrap();
    assert_eq!(editor.forms(&session).len(), 1);

    let mut values = BTreeMap::new();
    values.insert("name".to_owned(), PropertyValue::String("Latte".to_owned()));
    let valid = editor
        .validate_form(&session, &form_id, values.clone())
        .unwrap();
    assert!(valid.valid);
    assert!(valid.field_errors.is_empty());

    values.insert("name".to_owned(), PropertyValue::String(String::new()));
    let invalid = editor.validate_form(&session, &form_id, values).unwrap();
    assert!(!invalid.valid);
    assert!(invalid.field_errors.contains_key("name"));

    assert!(matches!(
        editor.validate_form(
            &session,
            &FormId::new("missing-form").unwrap(),
            BTreeMap::new()
        ),
        Err(ContentEditorError::FormMissing(_))
    ));
}

#[test]
fn t49_record_crud_goes_through_tracked_commands() {
    let mut session = editor_session();
    let editor = ContentEditor::new();
    let collection_id = CollectionId::new("menu-items").unwrap();

    block_on(editor.create_collection(
        &mut session,
        menu_collection(),
        OperationId::new("t49-record-create-collection").unwrap(),
        actor(),
        UndoGroupId::new("t49-record-create-collection").unwrap(),
    ))
    .unwrap();

    let mut values = BTreeMap::new();
    values.insert(
        "name".to_owned(),
        PropertyValue::String("Espresso".to_owned()),
    );
    let receipt = block_on(editor.create_record(
        &mut session,
        collection_id.clone(),
        ContentRecord {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: RecordId::new("record-1").unwrap(),
            values,
        },
        OperationId::new("t49-record-create").unwrap(),
        actor(),
        UndoGroupId::new("t49-record-create").unwrap(),
    ))
    .unwrap();
    assert_eq!(receipt.committed_revision.get(), 2);

    let collection = editor.collection(&session, &collection_id).unwrap();
    assert_eq!(collection.records.len(), 1);

    block_on(editor.delete_record(
        &mut session,
        collection_id.clone(),
        RecordId::new("record-1").unwrap(),
        OperationId::new("t49-record-delete").unwrap(),
        actor(),
        UndoGroupId::new("t49-record-delete").unwrap(),
    ))
    .unwrap();
    assert_eq!(
        editor
            .collection(&session, &collection_id)
            .unwrap()
            .records
            .len(),
        0
    );
}

#[test]
fn t51_composer_walks_the_welcome_to_thread_flow() {
    use studio_design::agent_conversation::{AgentModel, ConversationSurface, ReasoningEffort};

    let mut composer = ConversationComposer::new();
    assert_eq!(composer.surface(), ConversationSurface::Welcome);
    assert!(composer.active_transcript().is_empty());

    let model = AgentModel::new(
        "dlx",
        "DLX",
        "studio-small",
        "Studio Small",
        "fast local model",
    )
    .unwrap();
    composer.state_mut().catalog.add(model).unwrap();
    composer
        .select_model(studio_design::agent_conversation::ModelSelection {
            model: studio_design::agent_conversation::AgentModel::new(
                "dlx",
                "DLX",
                "studio-small",
                "Studio Small",
                "fast local model",
            )
            .unwrap(),
            effort: ReasoningEffort::High,
        })
        .unwrap();

    composer
        .set_draft("Add a menu editor to this screen")
        .unwrap();
    assert_eq!(composer.draft(), "Add a menu editor to this screen");

    let surface = composer
        .submit_draft(
            studio_design::agent_conversation::AgentMessageId::new("message-1").unwrap(),
            studio_design::agent_conversation::AgentRunId::new("run-1").unwrap(),
        )
        .unwrap();
    assert_eq!(surface, ConversationSurface::Thread);
    assert_eq!(composer.draft(), "");

    let transcript = composer.active_transcript();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].content, "Add a menu editor to this screen");

    composer
        .receive_assistant(
            studio_design::agent_conversation::AgentMessageId::new("message-2").unwrap(),
            "Added the menu editor card.",
            Vec::new(),
        )
        .unwrap();
    assert_eq!(composer.active_transcript().len(), 2);

    // Clearing the draft is allowed; submitting an empty draft is not.
    composer.set_draft("").unwrap();
    assert_eq!(composer.draft(), "");
    assert!(
        composer
            .submit_draft(
                studio_design::agent_conversation::AgentMessageId::new("message-3").unwrap(),
                studio_design::agent_conversation::AgentRunId::new("run-1").unwrap(),
            )
            .is_err()
    );
}

#[test]
fn t51_composer_rejects_unknown_models_and_runs() {
    use studio_design::agent_conversation::ModelSelection;

    let mut composer = ConversationComposer::new();
    let selection = ModelSelection {
        model: studio_design::agent_conversation::AgentModel::new(
            "ghost",
            "Ghost",
            "missing",
            "Missing",
            "not in catalog",
        )
        .unwrap(),
        effort: studio_design::agent_conversation::ReasoningEffort::Low,
    };
    assert!(composer.select_model(selection).is_err());

    composer.set_draft("Hello").unwrap();
    let unknown_run = studio_design::agent_conversation::AgentRunId::new("unknown-run").unwrap();
    // The domain requires the run identity to match the active run it starts;
    // submitting without a selected model must fail instead of inventing one.
    assert!(
        composer
            .submit_draft(
                studio_design::agent_conversation::AgentMessageId::new("message-x").unwrap(),
                unknown_run,
            )
            .is_err()
    );
    assert_eq!(
        composer.surface(),
        studio_design::agent_conversation::ConversationSurface::Welcome
    );
}

#[test]
fn t53_plugin_install_is_a_tracked_session_command() {
    let mut session = editor_session();
    let ux = PluginTemplateUx::new();
    let provenance = SourceProvenance {
        source_id: "registry".to_owned(),
        source_label: "Studio Registry".to_owned(),
        source_locator: None,
    };

    let receipt = block_on(ux.install(
        &mut session,
        &descriptor(),
        provenance.clone(),
        OperationId::new("t53-install").unwrap(),
        actor(),
        UndoGroupId::new("t53-install").unwrap(),
    ))
    .unwrap();
    assert_eq!(receipt.committed_revision.get(), 1);
    assert_eq!(receipt.undo_group_name, "Install plugin: POS Tools");

    let studio_design::DesignerQueryResult::Snapshot(snapshot) =
        session.query(studio_design::DesignerQuery::Snapshot)
    else {
        panic!("DesignerSession returned the wrong query result");
    };
    let plugin = snapshot
        .design
        .plugins
        .get(&PluginId::new("com.dlx.postools").unwrap())
        .unwrap();
    assert_eq!(plugin.version, "1.0.0");
    assert_eq!(plugin.publisher, "dlx");
    assert_eq!(plugin.provenance, provenance);
}

#[test]
fn t53_generated_settings_surface_and_tracked_edits() {
    let mut session = editor_session();
    let ux = PluginTemplateUx::new();
    let descriptor = descriptor();

    let surface = ux.settings_surface(&descriptor, &BTreeMap::new());
    assert_eq!(surface.plugin_name, "POS Tools");
    assert_eq!(surface.tabs.len(), 2);
    assert_eq!(surface.tabs[0].title, "General");
    assert_eq!(surface.tabs[0].fields[0].key.field_id, "printer-name");

    block_on(ux.install(
        &mut session,
        &descriptor,
        SourceProvenance {
            source_id: "registry".to_owned(),
            source_label: "Studio Registry".to_owned(),
            source_locator: None,
        },
        OperationId::new("t53-settings-install").unwrap(),
        actor(),
        UndoGroupId::new("t53-settings-install").unwrap(),
    ))
    .unwrap();

    let receipt = block_on(ux.change_setting(
        &mut session,
        &descriptor,
        printer_key(),
        Some(SettingValue::Text("Kitchen 1".to_owned())),
        OperationId::new("t53-setting-text").unwrap(),
        actor(),
        UndoGroupId::new("t53-setting-text").unwrap(),
    ))
    .unwrap();
    assert_eq!(receipt.committed_revision.get(), 2);

    let studio_design::DesignerQueryResult::Snapshot(snapshot) =
        session.query(studio_design::DesignerQuery::Snapshot)
    else {
        panic!("DesignerSession returned the wrong query result");
    };
    assert_eq!(
        snapshot.design.settings.get(&printer_key()),
        Some(&SettingValue::Text("Kitchen 1".to_owned()))
    );

    let out_of_range = block_on(ux.change_setting(
        &mut session,
        &descriptor,
        copies_key(),
        Some(SettingValue::Number("50".to_owned())),
        OperationId::new("t53-setting-range").unwrap(),
        actor(),
        UndoGroupId::new("t53-setting-range").unwrap(),
    ))
    .unwrap_err();
    assert!(matches!(
        out_of_range,
        PluginSurfaceError::Settings(studio_design::SettingsError::ValueInvalid { .. })
    ));

    let missing = {
        let project_id = match session.query(studio_design::DesignerQuery::SessionState) {
            studio_design::DesignerQueryResult::SessionState(state) => state.project_id,
            _ => panic!("DesignerSession returned the wrong query result"),
        };
        ux.settings_change_batch(
            &descriptor,
            SettingKey {
                plugin_id: PluginId::new("com.dlx.postools").unwrap(),
                group_id: "general".to_owned(),
                field_id: "unknown".to_owned(),
            },
            None,
            project_id,
            studio_design::RevisionId::INITIAL,
            OperationId::new("t53-setting-missing").unwrap(),
            actor(),
            UndoGroupId::new("t53-setting-missing").unwrap(),
        )
        .unwrap_err()
    };
    assert!(matches!(
        missing,
        PluginSurfaceError::Settings(studio_design::SettingsError::FieldMissing(_))
    ));
}
