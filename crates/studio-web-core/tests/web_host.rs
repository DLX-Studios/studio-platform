//! Browser bridge boundary and atomicity regressions.

use serde_json::{Value, json};
use studio_web_core::WebHost;

fn routes() -> String {
    json!({"schemaVersion":1,"routes":[
        {"id":"home","path":"/","source":"routes/+page.studio"},
        {"id":"live","path":"/live","source":"routes/live/+page.studio"},
        {"id":"stream","path":"/streams/:id","source":"routes/streams/[id]/+page.studio"}
    ]})
    .to_string()
}
fn timers() -> String {
    json!({"schemaVersion":1,"pages":[{"id":"home","timers":[{
        "id":"clock","tickNode":"clock-tick","completeNode":"clock-complete",
        "statusNode":"clock-status","intervalMs":1000,"completionDelayMs":700
    }]}]})
    .to_string()
}
fn empty_timers() -> &'static str {
    r#"{"schemaVersion":1,"pages":[]}"#
}
fn host() -> WebHost {
    WebHost::new(&routes(), &timers(), "/", 0.0).unwrap()
}
fn node(id: &str, kind: &str, props: Value, children: Vec<Value>) -> Value {
    Value::Object(serde_json::Map::from_iter([
        ("id".to_owned(), Value::String(id.to_owned())),
        ("kind".to_owned(), Value::String(kind.to_owned())),
        ("props".to_owned(), props),
        ("children".to_owned(), Value::Array(children)),
    ]))
}
fn root() -> Value {
    node(
        "home#root",
        "box",
        json!({}),
        vec![
            node("start", "button", json!({"label":"Start"}), vec![]),
            node("disabled", "button", json!({"enabled":false}), vec![]),
            node(
                "clock",
                "box",
                json!({"visible":false}),
                vec![
                    node("clock-tick", "button", json!({"label":"tick"}), vec![]),
                    node(
                        "clock-complete",
                        "button",
                        json!({"label":"complete"}),
                        vec![],
                    ),
                    node("clock-status", "text", json!({"text":"running"}), vec![]),
                ],
            ),
        ],
    )
}
fn mount(tree: Value, route: &str) -> String {
    let mut message = json!({"type":"mount","payload":{"protocol_version":1,"route":route}});
    message["payload"]["root"] = tree;
    message.to_string()
}
fn patch(sequence: u64, operations: Vec<Value>) -> String {
    let mut message = json!({"type":"patch","payload":{"sequence":sequence}});
    message["payload"]["operations"] = Value::Array(operations);
    message.to_string()
}
fn update(id: &str, property: &str, value: Value) -> Value {
    let mut operation = json!({"op":"update_prop","node_id":id,"property":property});
    operation["value"] = value;
    operation
}
fn value(encoded: String) -> Value {
    serde_json::from_slice(&encoded.into_bytes()).unwrap()
}

#[test]
fn navigation_reuses_rust_matching_history_and_rejects_invalid_commands_atomically() {
    let mut h = host();
    assert_eq!(
        value(h.current().unwrap()),
        json!({"route":"/","id":"home","params":{}})
    );
    let command = json!({"operation":"push","route":"/streams/42"}).to_string();
    assert_eq!(
        value(h.navigate(&command, 100.0).unwrap()),
        json!({"route":"/streams/42","id":"stream","params":{"id":"42"}})
    );
    h.accept(&mount(
        node("stream#root", "text", json!({"text":"stream"}), vec![]),
        "/streams/42",
    ))
    .unwrap();
    let before = h.current().unwrap();
    for command in [
        r#"{"operation":"push","route":"/missing"}"#,
        r#"{"operation":"push","route":"/live","extra":true}"#,
        r#"{"operation":"push","route":"/../live"}"#,
    ] {
        assert!(h.navigate(command, 200.0).is_err());
        assert_eq!(h.current().unwrap(), before);
    }
    assert_eq!(
        value(h.navigate(r#"{"operation":"pop"}"#, 300.0).unwrap())["route"],
        "/"
    );
}

#[test]
fn metadata_and_host_clock_are_bounded_before_creating_or_mutating_state() {
    for now in [-1.0, f64::NAN, f64::INFINITY, 0.5, 9_007_199_254_740_992.0] {
        assert!(WebHost::new(&routes(), empty_timers(), "/", now).is_err());
    }
    let bad = timers().replace("\"home\"", "\"missing\"");
    assert!(WebHost::new(&routes(), &bad, "/", 0.0).is_err());
    assert!(
        WebHost::new(
            &routes(),
            r#"{"schemaVersion":1,"pages":[],"extra":true}"#,
            "/",
            0.0
        )
        .is_err()
    );
    let mut h = host();
    assert!(
        h.navigate(r#"{"operation":"push","route":"/live"}"#, f64::NAN)
            .is_err()
    );
    assert_eq!(value(h.current().unwrap())["route"], "/");
}

#[test]
fn mount_is_closed_authorized_and_inputs_respect_ancestors_and_enabled_state() {
    let mut h = host();
    assert!(h.tree().is_err());
    assert!(!h.can_input("start", "pressed"));
    assert!(h.accept(&mount(root(), "/live")).is_err());
    let mut forged = root();
    forged["id"] = json!("live#root");
    assert!(h.accept(&mount(forged, "/")).is_err());
    assert_eq!(value(h.accept(&mount(root(), "/")).unwrap()), root());
    assert!(h.can_input("start", "pressed"));
    assert!(!h.can_input("start", "change"));
    assert!(!h.can_input("disabled", "pressed"));
    assert!(!h.can_input("clock-tick", "pressed"));
    assert!(!h.can_input("clock-status", "pressed"));
    assert!(h.timer_allowed("clock-tick", "pressed"));
    assert!(!h.timer_allowed("start", "pressed"));
    h.accept(&patch(
        1,
        vec![update("home#root", "visible", json!(false))],
    ))
    .unwrap();
    assert!(!h.can_input("start", "pressed"));
}

#[test]
fn invalid_wire_kinds_properties_and_combined_timer_patches_preserve_tree_and_sequence() {
    let mut h = host();
    h.accept(&mount(root(), "/")).unwrap();
    let before = h.tree().unwrap();
    let cases = [
        patch(
            1,
            vec![update("start", "html", json!("<script>bad</script>"))],
        ),
        patch(
            1,
            vec![
                update("start", "label", json!("valid")),
                update("clock", "visible", json!(true)),
            ],
        ),
        patch(
            1,
            vec![
                json!({"op":"replace_node","node_id":"start","node":node("start","slider",json!({}),vec![])}),
            ],
        ),
        patch(
            1,
            vec![
                json!({"op":"replace_node","node_id":"home#root","node":node("other","box",json!({}),vec![])}),
            ],
        ),
        patch(1, vec![update("clock-status", "text", json!("nonsense"))]),
    ];
    for case in cases {
        assert!(h.accept(&case).is_err(), "{case}");
        assert_eq!(h.tree().unwrap(), before);
    }
    h.accept(&patch(1, vec![update("start", "label", json!("good"))]))
        .unwrap();
    let after = h.tree().unwrap();
    assert!(
        h.accept(&patch(1, vec![update("start", "label", json!("replayed"))]))
            .is_err()
    );
    assert_eq!(h.tree().unwrap(), after);
    assert!(h.accept(r#"{"type":"action","payload":{}}"#).is_err());
    assert!(h.accept(&" ".repeat(1024 * 1024 + 1)).is_err());
}

#[test]
fn timer_mount_targets_must_belong_to_the_invisible_wrapper_with_correct_kinds() {
    let mut h = host();
    let mut wrong = root();
    wrong["children"][2]["children"][0]["kind"] = json!("text");
    wrong["children"][2]["children"][0]["props"] = json!({"text":"tick"});
    assert!(h.accept(&mount(wrong, "/")).is_err());
    let mut escaped = root();
    let moved = escaped["children"][2]["children"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    escaped["children"].as_array_mut().unwrap().push(moved);
    assert!(h.accept(&mount(escaped, "/")).is_err());
    assert!(h.tree().is_err());
    h.accept(&mount(root(), "/")).unwrap();
}

#[test]
fn timers_follow_retained_status_reset_and_route_exit_with_one_event_at_a_time() {
    let mut h = host();
    h.accept(&mount(root(), "/")).unwrap();
    assert_eq!(value(h.poll_one(999.0, "[]").unwrap()), Value::Null);
    assert_eq!(
        value(h.poll_one(1000.0, "[]").unwrap()),
        json!({"node_id":"clock-tick","event":"pressed"})
    );
    h.accept(&patch(
        1,
        vec![update("clock-status", "text", json!("complete"))],
    ))
    .unwrap();
    h.observe(1000.0, r#"["clock"]"#).unwrap();
    assert_eq!(
        value(h.poll_one(1699.0, r#"["clock"]"#).unwrap()),
        Value::Null
    );
    h.accept(&patch(
        2,
        vec![update("clock-status", "text", json!("running"))],
    ))
    .unwrap();
    h.observe(1699.0, "[]").unwrap();
    assert_eq!(value(h.poll_one(1700.0, "[]").unwrap()), Value::Null);
    assert_eq!(
        value(h.poll_one(2699.0, "[]").unwrap())["node_id"],
        "clock-tick"
    );
    h.navigate(r#"{"operation":"push","route":"/live"}"#, 2700.0)
        .unwrap();
    assert!(!h.timer_allowed("clock-tick", "pressed"));
    assert!(!h.can_input("start", "pressed"));
    assert_eq!(value(h.poll_one(10000.0, "[]").unwrap()), Value::Null);
    assert!(h.accept(&mount(root(), "/")).is_err());
    h.accept(&mount(
        node("live#root", "text", json!({"text":"live"}), vec![]),
        "/live",
    ))
    .unwrap();
    h.navigate(r#"{"operation":"pop"}"#, 10000.0).unwrap();
    h.accept(&mount(root(), "/")).unwrap();
    assert_eq!(value(h.poll_one(10999.0, "[]").unwrap()), Value::Null);
    assert_eq!(
        value(h.poll_one(11000.0, "[]").unwrap())["node_id"],
        "clock-tick"
    );
}

#[test]
fn completion_and_input_json_cannot_forge_timer_authority() {
    let mut h = host();
    h.accept(&mount(root(), "/")).unwrap();
    for completed in [
        r#"["unknown"]"#,
        r#"["clock","clock"]"#,
        r#"{"clock":true}"#,
        r#"["clock"]"#,
    ] {
        assert!(h.observe(1000.0, completed).is_err());
    }
    assert_eq!(
        value(h.poll_one(1000.0, "[]").unwrap())["node_id"],
        "clock-tick"
    );
    h.accept(&patch(
        1,
        vec![update("clock-status", "text", json!("complete"))],
    ))
    .unwrap();
    h.observe(1000.0, r#"["clock"]"#).unwrap();
    assert_eq!(
        value(h.poll_one(1700.0, r#"["clock"]"#).unwrap())["node_id"],
        "clock-complete"
    );
    assert_eq!(
        value(h.poll_one(100_000.0, r#"["clock"]"#).unwrap()),
        Value::Null
    );
}

#[test]
fn failed_navigation_mount_can_restore_prior_tree_history_and_timer_deadlines() {
    let mut h = host();
    h.accept(&mount(root(), "/")).unwrap();
    let prior = h.tree().unwrap();
    h.navigate(r#"{"operation":"push","route":"/live"}"#, 500.0)
        .unwrap();
    assert!(h.accept(&mount(root(), "/")).is_err());
    assert!(h.cancel_navigation());
    assert_eq!(h.tree().unwrap(), prior);
    assert_eq!(value(h.current().unwrap())["route"], "/");
    assert!(h.navigate(r#"{"operation":"pop"}"#, 600.0).is_err());
    assert_eq!(
        value(h.poll_one(1000.0, "[]").unwrap())["node_id"],
        "clock-tick"
    );
    assert!(!h.cancel_navigation());
}
