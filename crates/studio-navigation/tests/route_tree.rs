#![allow(missing_docs)]

use std::{cell::Cell, rc::Rc};

use studio_navigation::{RouteDefinition, RouteErrorCode, RouteResolution, RouteTree};

#[test]
fn static_nested_and_parameterized_routes_resolve_with_canonical_parameters() {
    let mut routes = RouteTree::new(
        [
            RouteDefinition::new("/catalog", || "catalog".to_owned()).unwrap(),
            RouteDefinition::new("/checkout/payment", || "payment".to_owned()).unwrap(),
            RouteDefinition::new("/receipts/:receipt_id", || "receipt".to_owned()).unwrap(),
        ],
        || "not-found".to_owned(),
    )
    .unwrap();

    assert_eq!(routes.resolve("/catalog").unwrap().screen(), "catalog");
    assert_eq!(
        routes.resolve("/checkout/payment").unwrap().screen(),
        "payment"
    );
    let receipt = routes.resolve("/receipts/receipt-42").unwrap();
    assert_eq!(receipt.screen(), "receipt");
    assert_eq!(receipt.pattern(), Some("/receipts/:receipt_id"));
    assert_eq!(receipt.params().get("receipt_id").unwrap(), "receipt-42");
}

#[test]
fn not_found_is_explicit_and_only_the_selected_screen_is_created() {
    let catalog_calls = Rc::new(Cell::new(0));
    let receipt_calls = Rc::new(Cell::new(0));
    let not_found_calls = Rc::new(Cell::new(0));
    let mut routes = RouteTree::new(
        [
            RouteDefinition::new("/catalog", {
                let calls = Rc::clone(&catalog_calls);
                move || {
                    calls.set(calls.get() + 1);
                    "catalog".to_owned()
                }
            })
            .unwrap(),
            RouteDefinition::new("/receipts/:id", {
                let calls = Rc::clone(&receipt_calls);
                move || {
                    calls.set(calls.get() + 1);
                    "receipt".to_owned()
                }
            })
            .unwrap(),
        ],
        {
            let calls = Rc::clone(&not_found_calls);
            move || {
                calls.set(calls.get() + 1);
                "not-found".to_owned()
            }
        },
    )
    .unwrap();
    assert_eq!(catalog_calls.get(), 0);
    assert_eq!(receipt_calls.get(), 0);
    assert_eq!(not_found_calls.get(), 0);

    assert!(matches!(
        routes.resolve("/unknown").unwrap(),
        RouteResolution::NotFound { .. }
    ));
    assert_eq!(catalog_calls.get(), 0);
    assert_eq!(receipt_calls.get(), 0);
    assert_eq!(not_found_calls.get(), 1);
}

#[test]
fn malformed_and_ambiguous_declarations_fail_before_matching() {
    for pattern in ["catalog", "/a//b", "/a/:", "/a/:id/:id", "/a?x=1"] {
        assert_eq!(
            RouteDefinition::<String>::new(pattern, String::new)
                .unwrap_err()
                .code(),
            RouteErrorCode::InvalidPattern
        );
    }

    let error = RouteTree::new(
        [
            RouteDefinition::new("/receipts/:id", || "a".to_owned()).unwrap(),
            RouteDefinition::new("/receipts/:receipt_id", || "b".to_owned()).unwrap(),
        ],
        || "not-found".to_owned(),
    )
    .unwrap_err();
    assert_eq!(error.code(), RouteErrorCode::AmbiguousDeclaration);
}

#[test]
fn root_catch_all_and_segment_specificity_share_one_resolution_rule() {
    let mut routes = RouteTree::new(
        [
            RouteDefinition::new("/*", || "fallback").unwrap(),
            RouteDefinition::new("/:section/new", || "parameter-first").unwrap(),
            RouteDefinition::new("/streams/:id", || "static-first").unwrap(),
            RouteDefinition::new("/streams/new", || "static").unwrap(),
            RouteDefinition::new("/", || "home").unwrap(),
        ],
        || "missing",
    )
    .unwrap();
    assert_eq!(*routes.resolve("/").unwrap().screen(), "home");
    assert_eq!(*routes.resolve("/streams/new").unwrap().screen(), "static");
    assert_eq!(
        *routes.resolve("/streams/42").unwrap().screen(),
        "static-first"
    );
    assert_eq!(
        *routes.resolve("/unknown/deep/path").unwrap().screen(),
        "fallback"
    );
    assert_eq!(
        routes
            .resolve("/streams/42")
            .unwrap()
            .params()
            .get("id")
            .unwrap(),
        "42"
    );

    let mut ambiguous_precedence = RouteTree::new(
        [
            RouteDefinition::new("/:section/new", || "parameter-first").unwrap(),
            RouteDefinition::new("/streams/:id", || "static-first").unwrap(),
        ],
        || "missing",
    )
    .unwrap();
    assert_eq!(
        *ambiguous_precedence
            .resolve("/streams/new")
            .unwrap()
            .screen(),
        "static-first"
    );
}

#[test]
fn malformed_wildcards_and_unsafe_concrete_paths_are_rejected() {
    for pattern in ["/a/*/b", "/a/**", "/a/:1id", "/a/:id/:id", "/a/../b"] {
        assert!(RouteDefinition::new(pattern, || ()).is_err(), "{pattern}");
    }
    let mut routes = RouteTree::new([RouteDefinition::new("/*", || ()).unwrap()], || ()).unwrap();
    for route in ["/a/*", "/a/:id", "/a/./b", "/a\\b", "/a//b"] {
        assert!(routes.resolve(route).is_err(), "{route}");
    }
}

#[test]
fn invalid_requested_paths_do_not_create_any_screen() {
    let mut routes = RouteTree::new(
        [RouteDefinition::new("/catalog", || "catalog".to_owned()).unwrap()],
        || "not-found".to_owned(),
    )
    .unwrap();
    for route in ["catalog", "/a//b", "/a/../b", "/a?x=1", "/a#fragment"] {
        assert_eq!(
            routes.resolve(route).unwrap_err().code(),
            RouteErrorCode::InvalidRoute
        );
    }
}
