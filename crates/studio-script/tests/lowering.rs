//! Typed IR lowering tests: structure parity with the parser semantic model
//! plus stable, source-linked diagnostics for constructs outside the subset.

use std::fs;
use std::path::Path;

use studio_script::ir::{IrNavigationOperation, IrNode, IrProperty, IrTriggerEvent};
use studio_script::lower::{
    CODE_IR_BEHAVIOR_SYNTAX, CODE_IR_DUPLICATE_TRIGGER, CODE_IR_UNKNOWN_KIND,
    CODE_IR_UNKNOWN_KEYWORD, CODE_IR_UNKNOWN_TARGET, CODE_IR_UNKNOWN_TRIGGER_NODE,
    CODE_IR_UNRESOLVED_TOKEN, CODE_IR_UNSUPPORTED_BINDING,
};
use studio_script::{compile, CompileError, Severity, STUDIO_SCRIPT_VERSION};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("lowering")
        .join(name);
    fs::read_to_string(path).expect("fixture should be readable")
}

#[test]
fn lowers_static_screens_and_navigation_into_versioned_ir() {
    let module = compile(&fixture("nav-app.studio")).expect("fixture should lower");

    assert_eq!(module.version, STUDIO_SCRIPT_VERSION);
    assert_eq!(module.screens.len(), 2);

    let home = &module.screens[0];
    assert_eq!(home.id, "home");
    assert_eq!(home.route, "/home");
    let IrNode::Element(root) = &home.root else {
        panic!("screen root should be an element");
    };
    assert_eq!(root.kind, "screen");
    assert_eq!(
        root.properties,
        vec![("title".to_owned(), IrProperty::String("Home".to_owned()))]
    );
    assert_eq!(root.children.len(), 3);
    let IrNode::Text(text) = &root.children[0] else {
        panic!("first child should be the text leaf");
    };
    assert_eq!(text.id, "home-text-1");
    assert_eq!(text.text, "Home");
    let IrNode::Element(button) = &root.children[1] else {
        panic!("second child should be an element");
    };
    assert_eq!(button.id, "open-detail");
    assert_eq!(button.kind, "button");

    let detail = &module.screens[1];
    assert_eq!(detail.route, "/detail");
    let IrNode::Element(detail_root) = &detail.root else {
        panic!("detail root should be an element");
    };
    assert_eq!(detail_root.kind, "app_bar");

    assert_eq!(module.actions.len(), 3);
    let first = &module.actions[0];
    assert_eq!(first.trigger_node_id, "open-detail");
    assert_eq!(first.trigger_event, IrTriggerEvent::Pressed);
    assert_eq!(
        first.operation,
        IrNavigationOperation::Push {
            route: "/detail".to_owned()
        }
    );
    assert_eq!(
        module.actions[1].operation,
        IrNavigationOperation::Replace {
            route: "/detail".to_owned()
        }
    );
    assert_eq!(
        module.actions[2].operation,
        IrNavigationOperation::Pop
    );
}

#[test]
fn document_tree_constructs_outside_the_subset_are_rejected_with_stable_codes() {
    let binding_error = compile("studio 1\n<List id=\"items\" items={$item.products} />\n")
        .expect_err("bindings must be rejected");
    let CompileError::Lower(error) = &binding_error else {
        panic!("parser should accept bounded bindings");
    };
    assert_eq!(error.diagnostics[0].code, CODE_IR_UNSUPPORTED_BINDING);
    assert_eq!(error.diagnostics[0].severity, Severity::Error);
    assert_eq!(error.diagnostics[0].span.start.line, 2);

    let token_error =
        compile("studio 1\n<Box id=\"box\" tone={token.colors.primary} />\n")
            .expect_err("tokens must be rejected");
    assert!(token_error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == CODE_IR_UNRESOLVED_TOKEN));

    let kind_error = compile("studio 1\n<Frobnicator id=\"frob\" />\n")
        .expect_err("unknown catalog kinds must be rejected");
    assert!(kind_error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == CODE_IR_UNKNOWN_KIND));
}

#[test]
fn behavior_statements_are_source_linked_and_stable() {
    let unknown_target = compile(
        "studio 1\n<script lang=\"studio\">\non pressed go push(/missing)\n</script>\n<Button id=\"go\" label=\"Go\" />\n",
    )
    .expect_err("unknown targets must be rejected");
    assert_eq!(
        unknown_target.diagnostics()[0].code,
        CODE_IR_UNKNOWN_TARGET
    );
    assert_eq!(unknown_target.diagnostics()[0].span.start.line, 3);

    let unknown_node = compile(
        "studio 1\n<script lang=\"studio\">\non pressed ghost pop()\n</script>\n<Button id=\"go\" label=\"Go\" />\n",
    )
    .expect_err("unknown trigger nodes must be rejected");
    assert_eq!(
        unknown_node.diagnostics()[0].code,
        CODE_IR_UNKNOWN_TRIGGER_NODE
    );

    let duplicate = compile(
        "studio 1\n<script lang=\"studio\">\non pressed go push(/go)\non pressed go pop()\n</script>\n<Screen id=\"go\" />\n",
    )
    .expect_err("duplicate triggers must be rejected");
    assert_eq!(duplicate.diagnostics()[0].code, CODE_IR_DUPLICATE_TRIGGER);
    assert_eq!(duplicate.diagnostics()[0].span.start.line, 4);

    let unknown_keyword = compile(
        "studio 1\n<script lang=\"studio\">\nevery morning push(/go)\n</script>\n<Screen id=\"go\" />\n",
    )
    .expect_err("unknown keywords must be rejected");
    assert_eq!(
        unknown_keyword.diagnostics()[0].code,
        CODE_IR_BEHAVIOR_SYNTAX
    );

    let unknown_action = compile(
        "studio 1\n<script lang=\"studio\">\non pressed go fling(/go)\n</script>\n<Screen id=\"go\" />\n",
    )
    .expect_err("unknown actions must be rejected");
    assert_eq!(
        unknown_action.diagnostics()[0].code,
        CODE_IR_UNKNOWN_KEYWORD
    );
}
