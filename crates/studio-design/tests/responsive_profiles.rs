#![allow(missing_docs)]

use studio_design::{
    Actor, ActorId, ActorKind, BreakpointProvenance, Command, CommandBatch, DefaultDesignerSession,
    DesignNode, DesignerQuery, DesignerQueryResult, DesignerSession, DeviceInput, DeviceProfile,
    DeviceProfileId, DeviceProfileMatrix, InMemoryDesignerPersistence, InputEnvironment,
    LayoutProperties, NodeId, NodeKind, OperationId, ProjectId, PropertyValue, ResponsiveValue,
    ResponsiveVariant, ResponsiveVariantId, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen,
    ScreenId, SelectionSnapshot, SessionContextUpdate, StudioDesign, StyleProperties, UndoGroupId,
    compare_profiles, resolve_node, select_variant,
};

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
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
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn node() -> DesignNode {
    DesignNode::primitive(
        NodeId::new("card").expect("valid node id"),
        "Card",
        NodeKind::Box,
    )
}

fn profile(id: &str, width: u32) -> DeviceProfile {
    DeviceProfile {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: DeviceProfileId::new(id).expect("valid profile id"),
        name: id.to_owned(),
        viewport: studio_design::Viewport { width, height: 800 },
        orientation: studio_design::Orientation::Portrait,
        safe_area: studio_design::Insets::default(),
        pixel_ratio: "1.0".to_owned(),
        input: DeviceInput {
            pointer: true,
            touch: false,
            keyboard: true,
            remote_focus: false,
        },
    }
}

#[test]
fn responsive_value_resolves_sparse_override_with_provenance() {
    let variant = ResponsiveVariantId::new("compact").expect("valid variant id");
    let mut value = ResponsiveValue::new(PropertyValue::Integer(24));
    value.set_override(variant.clone(), PropertyValue::Integer(12));

    assert_eq!(value.resolve(None).value, &PropertyValue::Integer(24));
    assert_eq!(value.resolve(None).provenance, BreakpointProvenance::Base);
    assert_eq!(
        value.resolve(Some(&variant)).value,
        &PropertyValue::Integer(12)
    );
    assert_eq!(
        value.resolve(Some(&variant)).provenance,
        BreakpointProvenance::Breakpoint(variant)
    );
}

#[test]
fn profile_matrix_is_complete_and_ordered() {
    let matrix = DeviceProfileMatrix::standard();
    assert_eq!(matrix.profiles.len(), 16);
    assert!(
        matrix
            .profiles
            .contains_key(&DeviceProfileId::new("phone-portrait").unwrap())
    );
    assert!(
        matrix
            .profiles
            .contains_key(&DeviceProfileId::new("4k-landscape").unwrap())
    );
    let ids = matrix
        .profiles
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn resolution_and_comparison_report_authored_vs_unintended_differences() {
    let compact = ResponsiveVariantId::new("compact").unwrap();
    let wide = ResponsiveVariantId::new("wide").unwrap();
    let mut design = StudioDesign::empty(ProjectId::new("project").unwrap(), "Test");
    design.responsive_variants = [
        (
            compact.clone(),
            ResponsiveVariant {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: compact.clone(),
                name: "Compact".to_owned(),
                minimum_width: None,
                maximum_width: Some(599),
                input: InputEnvironment::Pointer,
            },
        ),
        (
            wide.clone(),
            ResponsiveVariant {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: wide.clone(),
                name: "Wide".to_owned(),
                minimum_width: Some(600),
                maximum_width: None,
                input: InputEnvironment::Pointer,
            },
        ),
    ]
    .into_iter()
    .collect();
    let mut card = node();
    card.properties
        .insert("columns".to_owned(), PropertyValue::Integer(2));
    card.responsive_overrides.insert(
        compact.clone(),
        studio_design::ResponsiveNodeOverride {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            properties: [("columns".to_owned(), PropertyValue::Integer(1))]
                .into_iter()
                .collect(),
            layout: LayoutProperties::default(),
            style: StyleProperties::default(),
        },
    );
    design.nodes.insert(card.id.clone(), card.clone());

    let small = profile("small", 390);
    let large = profile("large", 1200);
    assert_eq!(select_variant(&design, &small), Some(compact));
    assert_eq!(select_variant(&design, &large), Some(wide));
    assert_eq!(
        resolve_node(&design, &card, &small).properties["columns"].value,
        Some(PropertyValue::Integer(1))
    );
    let report = compare_profiles(&design, &card, &small, &large);
    assert_eq!(report.differences.len(), 1);
    assert!(!report.differences[0].unintended);
}

#[test]
#[allow(clippy::too_many_lines)]
fn breakpoint_command_and_profile_switch_preserve_editor_context() {
    block_on(async {
        let project_id = ProjectId::new("project").unwrap();
        let root_id = NodeId::new("root").unwrap();
        let screen_id = ScreenId::new("screen").unwrap();
        let variant_id = ResponsiveVariantId::new("compact").unwrap();
        let mut design = StudioDesign::empty(project_id.clone(), "Test");
        let mut root = node();
        root.id = root_id.clone();
        design.nodes.insert(root_id.clone(), root);
        design.parents.insert(
            root_id.clone(),
            studio_design::NodeParent::Screen {
                screen_id: screen_id.clone(),
            },
        );
        design.screens.insert(
            screen_id.clone(),
            Screen {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: screen_id.clone(),
                name: "Main".to_owned(),
                route: "/".to_owned(),
                root_node_id: root_id.clone(),
            },
        );
        design.screen_order.push(screen_id);
        design.responsive_variants.insert(
            variant_id.clone(),
            ResponsiveVariant {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: variant_id.clone(),
                name: "Compact".to_owned(),
                minimum_width: None,
                maximum_width: Some(600),
                input: InputEnvironment::Any,
            },
        );
        let mut session = DefaultDesignerSession::create(
            InMemoryDesignerPersistence::default(),
            design,
            OperationId::new("create").unwrap(),
            Actor {
                id: ActorId::new("actor").unwrap(),
                kind: ActorKind::Human,
                display_name: "Designer".to_owned(),
            },
            UndoGroupId::new("create").unwrap(),
        )
        .await
        .unwrap();
        let before = session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                node_ids: vec![root_id.clone()],
                primary: Some(root_id.clone()),
            }),
            device_profile: Some(Some("phone-portrait".to_owned())),
            canvas: Some(studio_design::CanvasStateSnapshot {
                zoom: "1.25".to_owned(),
                pan_x: "8".to_owned(),
                pan_y: "-3".to_owned(),
            }),
            ..SessionContextUpdate::default()
        });
        let outcome = session
            .submit(CommandBatch {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                operation_id: OperationId::new("breakpoint").unwrap(),
                actor: Actor {
                    id: ActorId::new("actor").unwrap(),
                    kind: ActorKind::Human,
                    display_name: "Designer".to_owned(),
                },
                project_id,
                base_revision: RevisionId::INITIAL,
                undo_group_id: UndoGroupId::new("responsive").unwrap(),
                undo_group_name: "Responsive override".to_owned(),
                preconditions: Vec::new(),
                commands: vec![Command::SetBreakpointProperty {
                    node_id: root_id.clone(),
                    variant_id,
                    property: "columns".to_owned(),
                    value: Some(PropertyValue::Integer(1)),
                }],
            })
            .await;
        assert!(matches!(
            outcome,
            studio_design::CommandOutcome::Accepted(_)
        ));
        let DesignerQueryResult::SessionState(after) = session.query(DesignerQuery::SessionState)
        else {
            unreachable!()
        };
        assert_eq!(after.selection, before.selection);
        assert_eq!(after.device_profile, before.device_profile);
        assert_eq!(after.canvas, before.canvas);
        let DesignerQueryResult::ResponsiveInspector(inspected) =
            session.query(DesignerQuery::ResponsiveInspector {
                node_id: root_id,
                profile_id: DeviceProfileId::new("phone-portrait").unwrap(),
            })
        else {
            unreachable!()
        };
        assert_eq!(
            inspected[0].provenance,
            BreakpointProvenance::Breakpoint(ResponsiveVariantId::new("compact").unwrap())
        );
    });
}
