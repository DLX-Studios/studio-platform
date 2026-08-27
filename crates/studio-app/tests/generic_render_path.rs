//! Static guardrails for the renderer boundary.

#[test]
fn generic_renderer_does_not_encode_vertical_identifiers_or_behavior() {
    let source = include_str!("../src/foundation.rs");
    let forbidden = [
        concat!("order-", "pane"),
        concat!("catalog-", "pane"),
        concat!("add", "-"),
        concat!("checkout", "-", "route"),
        concat!("trusted", "-", "confirmation"),
    ];
    for identifier in forbidden {
        assert!(
            !source.contains(identifier),
            "generic renderer contains vertical identifier {identifier:?}"
        );
    }
}
