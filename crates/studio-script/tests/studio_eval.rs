//! US2 evaluator contract: mount projections and handler dispatches over the
//! reference fixtures behave identically to the declared template.

use std::collections::BTreeMap;

use studio_script::eval::{dispatch_event, mount_tree};
use studio_script::ir::{StudioExpression, StudioModule};
use studio_script::types::StudioValue;

fn compile_fixture(name: &str) -> StudioModule {
    let path = format!(
        "{}/tests/fixtures/ir-v2/{name}/source.studio",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path).expect("fixture readable");
    studio_script::compile_studio(&source, &format!("{name}.studio"), name)
        .expect("fixture compiles")
}

fn node_ids(node: &studio_protocol::UiNode, ids: &mut Vec<String>) {
    ids.push(node.id.clone());
    for child in &node.children {
        node_ids(child, ids);
    }
}

#[test]
fn mounts_project_declared_templates_with_unique_ids() {
    let module = compile_fixture("typed-events");
    let component = &module.components[0];
    let (tree, scope) = mount_tree(&module, component, &BTreeMap::new()).unwrap();

    assert_eq!(tree.route, "/typed-events");
    assert_eq!(tree.root.children.len(), 1);
    let card = &tree.root.children[0];
    // `Card` is a single-child container: the authored `Column` carries
    // both children so the tree validates with no implicit wrap node.
    assert_eq!(card.children.len(), 1, "card holds one column");
    let wrap = &card.children[0];
    assert_eq!(wrap.id, "events-col");
    assert_eq!(wrap.children.len(), 2, "whitespace text is skipped");

    let mut ids = Vec::new();
    node_ids(&tree.root, &mut ids);
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "retained node ids are unique");

    assert_eq!(
        scope.state.get("quantity"),
        Some(&StudioValue::Number(studio_script::types::NumberLiteral {
            text: "1".to_owned(),
            value: 1.0
        }))
    );
}

#[test]
fn dispatches_return_typed_emissions() {
    let module = compile_fixture("typed-events");
    let component = &module.components[0];
    let (_, scope) = mount_tree(&module, component, &BTreeMap::new()).unwrap();

    let emission = dispatch_event(
        component,
        &scope,
        "events-add",
        "pressed",
        &component.handlers,
    )
    .unwrap()
    .expect("handler fires");
    assert_eq!(emission.name, "add-to-order");
    assert_eq!(
        emission.payload.get("productId"),
        Some(&StudioValue::String("Latte".to_owned()))
    );
    assert_eq!(
        emission.payload.get("quantity"),
        Some(&StudioValue::Number(studio_script::types::NumberLiteral {
            text: "1".to_owned(),
            value: 1.0
        }))
    );

    assert_eq!(
        dispatch_event(
            component,
            &scope,
            "events-add",
            "changed",
            &component.handlers
        )
        .unwrap(),
        None,
        "unhandled event families stay silent"
    );
    assert_eq!(
        dispatch_event(
            component,
            &scope,
            "typed-events#missing",
            "pressed",
            &component.handlers
        )
        .unwrap(),
        None,
        "unknown nodes stay silent"
    );
}

#[test]
fn derived_slots_evaluate_before_projection() {
    let module = compile_fixture("state-derived");
    let component = &module.components[0];
    let (tree, scope) = mount_tree(&module, component, &BTreeMap::new()).unwrap();

    // quantity 1 * price 0 = 0.
    let total = find_text(&tree.root, "derived-total").expect("total node projected");
    assert_eq!(total, "0");
    assert!(scope.derived.contains_key("total"));
}

#[test]
fn keyed_collections_render_one_row_per_item() {
    let module = compile_fixture("keyed-each");
    let component = &module.components[0];
    let items = StudioValue::Array(vec![
        StudioValue::Record(
            [
                ("id".to_owned(), StudioValue::String("a".to_owned())),
                ("name".to_owned(), StudioValue::String("A".to_owned())),
            ]
            .into_iter()
            .collect(),
        ),
        StudioValue::Record(
            [
                ("id".to_owned(), StudioValue::String("b".to_owned())),
                ("name".to_owned(), StudioValue::String("B".to_owned())),
            ]
            .into_iter()
            .collect(),
        ),
    ]);
    let items = BTreeMap::from([("items".to_owned(), items)]);
    let (tree, _) = mount_tree(&module, component, &items).unwrap();
    let card = &tree.root.children[0];
    assert_eq!(
        card.children.len(),
        1,
        "the each block projects one wrapper"
    );
    assert_eq!(
        card.children[0].children.len(),
        2,
        "one row per item inside the wrapper"
    );
}

#[test]
fn missing_required_props_fail_before_projection() {
    let source = "<script>let { name } = $props();</script>\n<Card id=\"a\"><Text id=\"b\">{name}</Text></Card>";
    let module = studio_script::compile_studio(source, "required.studio", "required").unwrap();
    let component = &module.components[0];
    assert!(component.props[0].required);
    assert!(mount_tree(&module, component, &BTreeMap::new()).is_err());
}

fn find_text(node: &studio_protocol::UiNode, id: &str) -> Option<String> {
    if node.id == id {
        return Some(collect_text(node));
    }
    node.children.iter().find_map(|child| find_text(child, id))
}

/// Concatenate every text leaf under a node.
fn collect_text(node: &studio_protocol::UiNode) -> String {
    let mut text = node
        .props
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned();
    for child in &node.children {
        text.push_str(&collect_text(child));
    }
    text
}

#[test]
fn expressions_classify_to_the_closed_set() {
    // Every dynamic prop in the fixtures lowers to a closed variant.
    let module = compile_fixture("interpolation");
    let component = &module.components[0];
    let card = &component.template;
    assert!(!card.is_empty());
    let mut dynamics = 0usize;
    for node in card {
        count_dynamics(node, &mut dynamics);
    }
    assert!(dynamics > 0, "fixture exercises dynamic bindings");
}

fn count_dynamics(node: &studio_script::ir::TemplateNode, dynamics: &mut usize) {
    match node {
        studio_script::ir::TemplateNode::Component {
            props, children, ..
        } => {
            for prop in props {
                if matches!(prop.value, studio_script::ir::PropValue::Expression(_)) {
                    *dynamics += 1;
                }
            }
            for child in children {
                count_dynamics(child, dynamics);
            }
        }
        studio_script::ir::TemplateNode::Interpolation { expression, .. } => {
            assert!(matches!(
                expression,
                StudioExpression::ReadProp(_)
                    | StudioExpression::ReadState(_)
                    | StudioExpression::ReadDerived(_)
            ));
            *dynamics += 1;
        }
        studio_script::ir::TemplateNode::If {
            condition,
            consequent,
            alternate,
            ..
        } => {
            let _ = condition;
            for child in consequent.iter().chain(alternate.iter()) {
                count_dynamics(child, dynamics);
            }
        }
        studio_script::ir::TemplateNode::Each { body, fallback, .. } => {
            for child in body.iter().chain(fallback.iter()) {
                count_dynamics(child, dynamics);
            }
        }
        studio_script::ir::TemplateNode::Text { .. } => {}
    }
}

#[test]
fn implicit_wrap_keeps_dynamic_cards_mountable() {
    // Static violations fail at check time; dynamic branches (skipped by
    // the checker) still wrap at projection so the mount validates.
    let source = "<script>let { show = true } = $props();</script>\n<Card id=\"a\"><Text id=\"b\">B</Text>{#if show}<Text id=\"d\">D</Text>{/if}</Card>";
    let module = studio_script::compile_studio(source, "wrap.studio", "wrap").unwrap();
    let component = &module.components[0];
    let (tree, _) = mount_tree(&module, component, &BTreeMap::new()).unwrap();
    let card = &tree.root.children[0];
    assert_eq!(card.children.len(), 1, "one implicit wrapper");
    assert_eq!(card.children[0].id, "a#wrap");
    // The wrapped tree validates under the protocol.
    let mount = studio_protocol::GuestMessage::Mount(tree);
    studio_protocol::decode_guest_message(
        &serde_json::to_vec(&mount).expect("mount serializes"),
        studio_protocol::ProtocolLimits::default(),
    )
    .expect("wrapped mount validates");
}
