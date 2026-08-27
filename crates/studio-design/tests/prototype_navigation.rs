#![allow(missing_docs)]

use studio_design::{
    DesignNode, Interaction, InteractionAction, InteractionEvent, InteractionGraph, InteractionId,
    InteractionSource, NavigationGraph, NavigationMode, NodeId, NodeParent, ProjectId,
    PropertyValue, PrototypeEffect, PrototypeEvent, PrototypeSession, STUDIO_DESIGN_SCHEMA_VERSION,
    Screen, ScreenId, StudioDesign,
};
use studio_protocol::NodeKind;

fn project() -> ProjectId {
    ProjectId::new("prototype-project").unwrap()
}

fn node(value: &str) -> NodeId {
    NodeId::new(value).unwrap()
}

fn screen(value: &str) -> ScreenId {
    ScreenId::new(value).unwrap()
}

fn interaction(value: &str) -> InteractionId {
    InteractionId::new(value).unwrap()
}

#[allow(clippy::too_many_lines)]
fn design() -> StudioDesign {
    let home = screen("home");
    let detail = screen("detail");
    let home_root = node("home-root");
    let open = node("open");
    let detail_root = node("detail-root");
    let back = node("back");
    let mut home_node = DesignNode::primitive(home_root.clone(), "Home", NodeKind::Box);
    home_node.children = vec![open.clone()];
    let mut open_node = DesignNode::primitive(open.clone(), "Open", NodeKind::Button);
    open_node.interaction_ids = vec![interaction("open-detail")];
    let mut detail_node = DesignNode::primitive(detail_root.clone(), "Detail", NodeKind::Box);
    detail_node.children = vec![back.clone()];
    let mut back_node = DesignNode::primitive(back.clone(), "Back", NodeKind::Button);
    back_node.interaction_ids = vec![interaction("back-home")];
    let mut result = StudioDesign::empty(project(), "Prototype");
    result.nodes = [
        (home_root.clone(), home_node),
        (open.clone(), open_node),
        (detail_root.clone(), detail_node),
        (back.clone(), back_node),
    ]
    .into_iter()
    .collect();
    result.parents = [
        (
            home_root.clone(),
            NodeParent::Screen {
                screen_id: home.clone(),
            },
        ),
        (open.clone(), NodeParent::Node { node_id: home_root }),
        (
            detail_root.clone(),
            NodeParent::Screen {
                screen_id: detail.clone(),
            },
        ),
        (
            back.clone(),
            NodeParent::Node {
                node_id: detail_root,
            },
        ),
    ]
    .into_iter()
    .collect();
    result.screens = [
        (
            home.clone(),
            Screen {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: home.clone(),
                name: "Home".to_owned(),
                route: "/".to_owned(),
                root_node_id: node("home-root"),
            },
        ),
        (
            detail.clone(),
            Screen {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: detail.clone(),
                name: "Detail".to_owned(),
                route: "/detail".to_owned(),
                root_node_id: node("detail-root"),
            },
        ),
    ]
    .into_iter()
    .collect();
    result.screen_order = vec![home.clone(), detail.clone()];
    result.interactions.insert(
        interaction("open-detail"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("open-detail"),
            source: InteractionSource {
                node_id: node("open"),
                event: InteractionEvent::Pressed,
            },
            action: InteractionAction::Navigate {
                screen_id: detail,
                mode: NavigationMode::Push,
            },
        },
    );
    result.interactions.insert(
        interaction("back-home"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("back-home"),
            source: InteractionSource {
                node_id: node("back"),
                event: InteractionEvent::Pressed,
            },
            action: InteractionAction::Navigate {
                screen_id: home,
                mode: NavigationMode::PopTo,
            },
        },
    );
    result
}

#[test]
fn prototype_navigation_is_deterministic_and_design_immutable() {
    let source = design();
    let before = source.clone();
    let mut first = PrototypeSession::new(&source).unwrap();
    let mut second = PrototypeSession::new(&source).unwrap();

    let open = PrototypeEvent {
        node_id: node("open"),
        event: InteractionEvent::Pressed,
    };
    let first_open = first.dispatch(open.clone());
    let second_open = second.dispatch(open);
    assert_eq!(first_open, second_open);
    assert_eq!(first.active_route(), Some("/detail"));
    assert!(matches!(
        first_open.trace[0].effect,
        PrototypeEffect::Navigate { .. }
    ));

    let back = first.dispatch_event(node("back"), InteractionEvent::Pressed);
    assert_eq!(back.state.active_screen_id, Some(screen("home")));
    assert_eq!(
        source, before,
        "prototype dispatch must never mutate Studio Design"
    );
}

#[test]
fn prototype_local_effects_are_ephemeral_and_traceable() {
    let mut source = design();
    let target = node("open");
    source.interactions.insert(
        interaction("local-effects"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("local-effects"),
            source: InteractionSource {
                node_id: target.clone(),
                event: InteractionEvent::Changed,
            },
            action: InteractionAction::Sequence {
                interaction_ids: vec![interaction("set-title"), interaction("toggle")],
            },
        },
    );
    source
        .nodes
        .get_mut(&target)
        .expect("test source node")
        .interaction_ids
        .push(interaction("local-effects"));
    source.interactions.insert(
        interaction("set-title"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("set-title"),
            source: InteractionSource {
                node_id: target.clone(),
                event: InteractionEvent::Changed,
            },
            action: InteractionAction::SetProperty {
                node_id: target.clone(),
                property: "label".to_owned(),
                value: PropertyValue::String("Preview".to_owned()),
            },
        },
    );
    source.interactions.insert(
        interaction("toggle"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("toggle"),
            source: InteractionSource {
                node_id: target.clone(),
                event: InteractionEvent::Changed,
            },
            action: InteractionAction::ToggleVisibility {
                node_id: target.clone(),
            },
        },
    );
    let mut prototype = PrototypeSession::new(&source).unwrap();
    let result = prototype.dispatch_event(target.clone(), InteractionEvent::Changed);
    assert_eq!(
        prototype.property(&target, "label"),
        Some(PropertyValue::String("Preview".to_owned()))
    );
    assert_eq!(prototype.is_visible(&target), Some(false));
    assert_eq!(result.interaction_ids[0], interaction("local-effects"));
    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(source.nodes[&target].properties.get("label"), None);
}

#[test]
fn graph_diagnostics_link_missing_targets_and_cycles_to_interactions() {
    let mut source = design();
    source.interactions.insert(
        interaction("missing"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("missing"),
            source: InteractionSource {
                node_id: node("open"),
                event: InteractionEvent::Pressed,
            },
            action: InteractionAction::Navigate {
                screen_id: screen("unknown"),
                mode: NavigationMode::Push,
            },
        },
    );
    source.interactions.insert(
        interaction("cycle-a"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("cycle-a"),
            source: InteractionSource {
                node_id: node("open"),
                event: InteractionEvent::Focused,
            },
            action: InteractionAction::Sequence {
                interaction_ids: vec![interaction("cycle-b")],
            },
        },
    );
    source.interactions.insert(
        interaction("cycle-b"),
        Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction("cycle-b"),
            source: InteractionSource {
                node_id: node("open"),
                event: InteractionEvent::Focused,
            },
            action: InteractionAction::Sequence {
                interaction_ids: vec![interaction("cycle-a")],
            },
        },
    );
    let graph = NavigationGraph::from_design(&source);
    assert!(!graph.is_valid());
    assert!(graph.diagnostics().iter().any(|diagnostic| {
        diagnostic.interaction_id == Some(interaction("missing"))
            && diagnostic.code == "DESIGN_INTERACTION_TARGET_MISSING"
    }));
    assert!(graph.diagnostics().iter().any(|diagnostic| {
        diagnostic.interaction_id == Some(interaction("cycle-a"))
            && diagnostic.code == "DESIGN_INTERACTION_CYCLE"
    }));
    let interaction_graph = InteractionGraph::from_design(&source);
    assert_eq!(
        interaction_graph
            .inspect(&node("open"), InteractionEvent::Pressed)
            .len(),
        1
    );
}
