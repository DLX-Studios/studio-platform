#![allow(
    missing_docs,
    clippy::pedantic,
    clippy::manual_let_else,
    clippy::field_reassign_with_default
)]

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, BindingId, CollectionId, Command, CommandBatch, CommandOutcome,
    ContentBinding, ContentCollection, ContentCollectionSchema, ContentFieldKind,
    ContentFieldSchema, ContentFixture, ContentRecord, DefaultDesignerSession, DesignerQuery,
    DesignerQueryResult, DesignerSession, FixtureKind, FormDefinition, FormFieldSchema, FormId,
    InMemoryDesignerPersistence, NodeId, OperationId, ProjectId, PropertyValue, RecordId,
    RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, StudioDesign, UndoGroupId,
};
use studio_protocol::NodeKind;

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

fn project_id() -> ProjectId {
    ProjectId::new("project-49").unwrap()
}
fn collection_id() -> CollectionId {
    CollectionId::new("products").unwrap()
}
fn binding_id() -> BindingId {
    BindingId::new("binding-title").unwrap()
}
fn form_id() -> FormId {
    FormId::new("form-checkout").unwrap()
}
fn operation_id(v: &str) -> OperationId {
    OperationId::new(v).unwrap()
}
fn undo_group_id(v: &str) -> UndoGroupId {
    UndoGroupId::new(v).unwrap()
}
fn actor() -> Actor {
    Actor {
        id: ActorId::new("actor-user").unwrap(),
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    }
}
fn node_id(v: &str) -> NodeId {
    NodeId::new(v).unwrap()
}
fn record_id(v: &str) -> RecordId {
    RecordId::new(v).unwrap()
}

fn empty_design_with_nodes() -> StudioDesign {
    let mut design = StudioDesign::empty(project_id(), "Test");
    // Minimal node for bindings to target
    let nid = node_id("list");
    design.nodes.insert(
        nid.clone(),
        studio_design::DesignNode::primitive(nid.clone(), "List", NodeKind::Box),
    );
    design.parents.insert(
        nid.clone(),
        studio_design::NodeParent::Node {
            node_id: node_id("root"),
        },
    );
    // root
    let root = node_id("root");
    let mut root_node = studio_design::DesignNode::primitive(root.clone(), "Root", NodeKind::Box);
    root_node.children = vec![nid.clone()];
    design.nodes.insert(root.clone(), root_node);
    design.parents.insert(
        root.clone(),
        studio_design::NodeParent::Screen {
            screen_id: studio_design::ScreenId::new("screen-main").unwrap(),
        },
    );
    design.screens.insert(
        studio_design::ScreenId::new("screen-main").unwrap(),
        studio_design::Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: studio_design::ScreenId::new("screen-main").unwrap(),
            name: "Main".to_owned(),
            route: "/".to_owned(),
            root_node_id: root.clone(),
        },
    );
    design
        .screen_order
        .push(studio_design::ScreenId::new("screen-main").unwrap());
    design
}

fn make_collection() -> ContentCollection {
    let mut fields = BTreeMap::new();
    fields.insert(
        "title".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::String,
            required: true,
        },
    );
    fields.insert(
        "price".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::Integer,
            required: true,
        },
    );
    ContentCollection {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: collection_id(),
        name: "Products".to_owned(),
        schema: ContentCollectionSchema {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            fields,
        },
        records: BTreeMap::new(),
        fixture: ContentFixture::default(),
    }
}

fn make_record(id: &str, title: &str, price: i64) -> ContentRecord {
    let mut values = BTreeMap::new();
    values.insert("title".to_owned(), PropertyValue::String(title.to_owned()));
    values.insert("price".to_owned(), PropertyValue::Integer(price));
    ContentRecord {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: record_id(id),
        values,
    }
}

fn batch(base: RevisionId, op: &str, group: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(op),
        actor: actor(),
        project_id: project_id(),
        base_revision: base,
        undo_group_id: undo_group_id(group),
        undo_group_name: group.replace('-', " "),
        preconditions: Vec::new(),
        commands,
    }
}
fn assert_accepted(outcome: CommandOutcome) -> studio_design::CommandReceipt {
    match outcome {
        CommandOutcome::Accepted(r) => r,
        other => panic!("expected accepted, got {other:?}"),
    }
}
#[test]
fn list_bound_to_collection_renders_identically_across_all_fixture_states() {
    // Deterministic preview contract: same collection+fixture => same preview every time.
    let mut collection = make_collection();
    collection
        .records
        .insert(record_id("r1"), make_record("r1", "Alpha", 10));
    collection
        .records
        .insert(record_id("r2"), make_record("r2", "Beta", 20));
    // Edge fixture uses custom records; error fixture uses message.
    collection.fixture = ContentFixture {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        kind: FixtureKind::Populated,
        edge_records: vec![make_record("edge-1", "EdgeItem", 99)],
        error_message: Some("fixture error".to_owned()),
    };

    for fixture in [
        FixtureKind::Empty,
        FixtureKind::Loading,
        FixtureKind::Error,
        FixtureKind::Populated,
        FixtureKind::Edge,
    ] {
        let a = studio_design::preview_collection(&collection, Some(fixture));
        let b = studio_design::preview_collection(&collection, Some(fixture));
        assert_eq!(a, b, "fixture {fixture:?} must be deterministic");

        match fixture {
            FixtureKind::Empty => {
                assert!(a.records.is_empty());
                assert!(!a.is_loading);
                assert!(!a.is_error);
            }
            FixtureKind::Loading => {
                assert!(a.records.is_empty());
                assert!(a.is_loading);
            }
            FixtureKind::Error => {
                assert!(a.records.is_empty());
                assert!(a.is_error);
                assert!(a.error_message.is_some());
            }
            FixtureKind::Populated => {
                assert_eq!(a.records.len(), 2);
                assert_eq!(a.records[0].id, record_id("r1"));
            }
            FixtureKind::Edge => {
                assert_eq!(a.records.len(), 1);
                assert_eq!(a.records[0].id, record_id("edge-1"));
            }
        }

        // Repeated binding resolves deterministically against each fixture's record set.
        let binding = ContentBinding {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: binding_id(),
            node_id: node_id("list"),
            property: "text".to_owned(),
            source: studio_design::BindingSource {
                collection_id: collection_id(),
                field: "title".to_owned(),
            },
            expected_kind: ContentFieldKind::String,
            fallback: Some(PropertyValue::String("Fallback".to_owned())),
            repeated: true,
        };
        // Render the repeated list: one entry per preview record, fallback when no record.
        let rendered = if a.records.is_empty() {
            vec![studio_design::resolve_binding_value(&binding, &collection, None).unwrap()]
        } else {
            a.records
                .iter()
                .map(|r| {
                    studio_design::resolve_binding_value(&binding, &collection, Some(r)).unwrap()
                })
                .collect::<Vec<_>>()
        };
        // All fixture states produce a deterministic, non-panicking render path.
        let rendered2 = if b.records.is_empty() {
            vec![studio_design::resolve_binding_value(&binding, &collection, None).unwrap()]
        } else {
            b.records
                .iter()
                .map(|r| {
                    studio_design::resolve_binding_value(&binding, &collection, Some(r)).unwrap()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(rendered, rendered2);
    }
}

#[test]
fn binding_type_mismatch_is_build_error_unless_valid_fallback_declared() {
    let mut design = empty_design_with_nodes();
    let mut collection = make_collection();
    collection
        .records
        .insert(record_id("r1"), make_record("r1", "X", 5));
    design.collections.insert(collection_id(), collection);
    // Binding expects String but field "price" is Integer, no fallback => build error.
    let bad_binding = ContentBinding {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: binding_id(),
        node_id: node_id("list"),
        property: "text".to_owned(),
        source: studio_design::BindingSource {
            collection_id: collection_id(),
            field: "price".to_owned(),
        },
        expected_kind: ContentFieldKind::String,
        fallback: None,
        repeated: false,
    };
    design.bindings.insert(binding_id(), bad_binding.clone());
    let errors = studio_design::build_blocking_binding_errors(&design);
    assert!(
        !errors.is_empty(),
        "mismatch without fallback must be build error"
    );
    assert!(errors.iter().any(|d| d.code == "CONTENT_BUILD_BLOCKED"));

    // Same mismatch but with a valid typed fallback => not a build error (only a warning diagnostic).
    let mut design2 = design.clone();
    let good_binding = ContentBinding {
        fallback: Some(PropertyValue::String("fallback".to_owned())),
        ..bad_binding.clone()
    };
    design2.bindings.insert(binding_id(), good_binding);
    let errors2 = studio_design::build_blocking_binding_errors(&design2);
    assert!(
        errors2.is_empty(),
        "mismatch with valid fallback must not be build error"
    );
    // But there is still a warning diagnostic (not build-blocking).
    let diags = studio_design::binding_diagnostics(&design2);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "CONTENT_BINDING_TYPE_MISMATCH_WITH_FALLBACK")
    );
}

#[test]
fn form_validation_runs_declaratively_in_prototype_mode() {
    let mut fields = BTreeMap::new();
    fields.insert(
        "email".to_owned(),
        FormFieldSchema {
            kind: ContentFieldKind::String,
            required: true,
            minimum_length: Some(5),
            maximum_length: Some(100),
            pattern: Some(r"^[^@]+@[^@]+\.[^@]+$".to_owned()),
            minimum_value: None,
            maximum_value: None,
        },
    );
    fields.insert(
        "age".to_owned(),
        FormFieldSchema {
            kind: ContentFieldKind::Integer,
            required: true,
            minimum_length: None,
            maximum_length: None,
            pattern: None,
            minimum_value: Some("18".to_owned()),
            maximum_value: Some("120".to_owned()),
        },
    );
    let form = FormDefinition {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: form_id(),
        name: "Checkout".to_owned(),
        fields,
        target_collection_id: None,
    };
    // Valid values
    let mut valid = BTreeMap::new();
    valid.insert(
        "email".to_owned(),
        PropertyValue::String("user@example.com".to_owned()),
    );
    valid.insert("age".to_owned(), PropertyValue::Integer(30));
    let result = studio_design::validate_form_values(&form, &valid);
    assert!(result.valid);
    assert!(result.field_errors.is_empty());

    // Missing required
    let mut missing = BTreeMap::new();
    missing.insert("age".to_owned(), PropertyValue::Integer(30));
    let result2 = studio_design::validate_form_values(&form, &missing);
    assert!(!result2.valid);
    assert!(result2.field_errors.contains_key("email"));

    // Pattern fail + too short + numeric bounds
    let mut invalid = BTreeMap::new();
    invalid.insert("email".to_owned(), PropertyValue::String("bad".to_owned()));
    invalid.insert("age".to_owned(), PropertyValue::Integer(10));
    let result3 = studio_design::validate_form_values(&form, &invalid);
    assert!(!result3.valid);
    assert!(result3.field_errors.contains_key("email"));
    assert!(result3.field_errors.contains_key("age"));

    // Deterministic: same inputs => same validation result
    let again = studio_design::validate_form_values(&form, &invalid);
    assert_eq!(result3, again);
}

#[test]
fn collection_schema_change_surfaces_affected_bindings_as_diagnostics() {
    let mut design = empty_design_with_nodes();
    let collection = make_collection();
    design
        .collections
        .insert(collection_id(), collection.clone());
    let binding = ContentBinding {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: binding_id(),
        node_id: node_id("list"),
        property: "text".to_owned(),
        source: studio_design::BindingSource {
            collection_id: collection_id(),
            field: "title".to_owned(),
        },
        expected_kind: ContentFieldKind::String,
        fallback: None,
        repeated: false,
    };
    design.bindings.insert(binding_id(), binding);
    // Change schema: remove "title"
    let mut new_fields = BTreeMap::new();
    new_fields.insert(
        "price".to_owned(),
        ContentFieldSchema {
            kind: ContentFieldKind::Integer,
            required: true,
        },
    );
    let new_schema = ContentCollectionSchema {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        fields: new_fields.clone(),
    };
    let diags = studio_design::schema_change_binding_diagnostics(
        &design,
        &collection_id(),
        &collection.schema,
        &new_schema,
    );
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.code == "CONTENT_SCHEMA_FIELD_REMOVED" && d.binding_id == Some(binding_id())));

    // Via the session: applying the schema update still commits, but diagnostics surface.
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = DefaultDesignerSession::create(
            persistence.clone(),
            empty_design_with_nodes(),
            operation_id("create"),
            actor(),
            undo_group_id("create"),
        )
        .await
        .unwrap();
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "create-collection",
                    "content",
                    vec![Command::CreateCollection {
                        collection: make_collection(),
                    }],
                ))
                .await,
        );
        // Bind before schema change
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::new(1),
                    "bind",
                    "content",
                    vec![Command::UpsertBinding {
                        binding: ContentBinding {
                            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                            id: binding_id(),
                            node_id: node_id("list"),
                            property: "text".to_owned(),
                            source: studio_design::BindingSource {
                                collection_id: collection_id(),
                                field: "title".to_owned(),
                            },
                            expected_kind: ContentFieldKind::String,
                            fallback: None,
                            repeated: false,
                        },
                    }],
                ))
                .await,
        );
        let new_schema2 = ContentCollectionSchema {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            fields: new_fields,
        };
        let outcome = session
            .submit(batch(
                RevisionId::new(2),
                "update-schema",
                "content",
                vec![Command::UpdateCollectionSchema {
                    collection_id: collection_id(),
                    schema: new_schema2.clone(),
                }],
            ))
            .await;
        // Schema update that would orphan bindings is currently rejected atomically; if that policy changes,
        // diagnostics would instead surface. Either behavior must not lose the error signal.
        // Our engine rejects schema changes that would invalidate existing records/break bindings orphan.
        // But binding orphan due to field removal without records is allowed; diagnostics should still appear via query.
        match outcome {
            CommandOutcome::Accepted(_) => {
                let diags = match session.query(DesignerQuery::Diagnostics) {
                    DesignerQueryResult::Diagnostics(d) => d,
                    _ => panic!(),
                };
                assert!(
                    diags.iter().any(|d| d.binding_id == Some(binding_id())),
                    "orphaned binding must surface as diagnostic: {diags:?}"
                );
            }
            CommandOutcome::Rejected(diags) => {
                assert!(
                    diags
                        .iter()
                        .any(|d| d.collection_id == Some(collection_id())),
                    "schema break should reject with collection diagnostic"
                );
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    });
}

#[test]
fn crud_via_session_is_atomic_and_queryable_and_undoable() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = DefaultDesignerSession::create(
            persistence,
            empty_design_with_nodes(),
            operation_id("create"),
            actor(),
            undo_group_id("create"),
        )
        .await
        .unwrap();

        // Create collection
        let collection = make_collection();
        let receipt = assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "op1",
                    "content",
                    vec![Command::CreateCollection {
                        collection: collection.clone(),
                    }],
                ))
                .await,
        );
        assert_eq!(receipt.committed_revision, RevisionId::new(1));
        let cols = match session.query(DesignerQuery::Collections) {
            DesignerQueryResult::Collections(v) => v,
            _ => panic!(),
        };
        assert_eq!(cols.len(), 1);

        // Create record
        let r1 = make_record("r1", "Hello", 42);
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::new(1),
                    "op2",
                    "content",
                    vec![Command::CreateRecord {
                        collection_id: collection_id(),
                        record: r1.clone(),
                    }],
                ))
                .await,
        );
        let col = match session.query(DesignerQuery::Collection {
            collection_id: collection_id(),
        }) {
            DesignerQueryResult::Collection(c) => c.unwrap(),
            _ => panic!(),
        };
        assert_eq!(col.records.len(), 1);

        // Preview in populated fixture returns the record
        let preview = match session.query(DesignerQuery::Preview {
            collection_id: collection_id(),
            fixture: Some(FixtureKind::Populated),
        }) {
            DesignerQueryResult::Preview(p) => p.unwrap(),
            _ => panic!(),
        };
        assert_eq!(preview.records.len(), 1);

        // Update fixture to error and verify preview deterministically shows error
        let mut err_fixture = ContentFixture::default();
        err_fixture.kind = FixtureKind::Error;
        err_fixture.error_message = Some("simulated error".to_owned());
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::new(2),
                    "op3",
                    "content",
                    vec![Command::SetFixture {
                        collection_id: collection_id(),
                        fixture: err_fixture.clone(),
                    }],
                ))
                .await,
        );
        let preview_err = match session.query(DesignerQuery::Preview {
            collection_id: collection_id(),
            fixture: None,
        }) {
            DesignerQueryResult::Preview(p) => p.unwrap(),
            _ => panic!(),
        };
        assert!(preview_err.is_error);
        assert_eq!(preview_err.fixture, FixtureKind::Error);

        // Form CRUD + declarative validation via session query
        let mut form_fields = BTreeMap::new();
        form_fields.insert(
            "name".to_owned(),
            FormFieldSchema {
                kind: ContentFieldKind::String,
                required: true,
                minimum_length: Some(1),
                maximum_length: Some(50),
                pattern: None,
                minimum_value: None,
                maximum_value: None,
            },
        );
        let form = FormDefinition {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: form_id(),
            name: "Contact".to_owned(),
            fields: form_fields,
            target_collection_id: Some(collection_id()),
        };
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::new(3),
                    "op4",
                    "content",
                    vec![Command::UpsertForm { form: form.clone() }],
                ))
                .await,
        );
        let forms = match session.query(DesignerQuery::Forms) {
            DesignerQueryResult::Forms(v) => v,
            _ => panic!(),
        };
        assert_eq!(forms.len(), 1);
        // Prototype validation
        let mut values = BTreeMap::new();
        values.insert("name".to_owned(), PropertyValue::String(String::new()));
        let validation = match session.query(DesignerQuery::ValidateForm {
            form_id: form_id(),
            values,
        }) {
            DesignerQueryResult::FormValidation(r) => r,
            _ => panic!(),
        };
        assert!(!validation.valid);
    });
}
