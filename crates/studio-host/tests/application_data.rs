#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::json;
use studio_host::{
    ApplicationDataErrorCode, ApplicationDataGuestApi, ApplicationDataHost, CollectionDeclaration,
    Durability, EmbeddedLocalStore, FieldDeclaration, FieldType, ForbiddenDataOperation,
    GuestDataRequest, LocalStore, PatchOperation, RecordId, RecordSchema,
};
use studio_security::{PluginPrincipal, TrustMode};
use tempfile::tempdir;

fn principal(publisher: &str, key: &str, application: &str) -> PluginPrincipal {
    PluginPrincipal::new_verified(
        publisher,
        key,
        application,
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .expect("fixture principal is valid")
}

fn catalog_declaration() -> CollectionDeclaration {
    CollectionDeclaration::new(
        "catalog",
        RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "name".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "price_cents".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
                (
                    "category".to_owned(),
                    FieldDeclaration::optional(FieldType::String),
                ),
                (
                    "available".to_owned(),
                    FieldDeclaration::required(FieldType::Boolean),
                ),
            ]),
        )
        .expect("fixture schema is valid"),
    )
    .expect("fixture collection is valid")
}

fn cart_declaration() -> CollectionDeclaration {
    CollectionDeclaration::new(
        "cart",
        RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "catalog_item_id".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "quantity".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
            ]),
        )
        .expect("fixture schema is valid"),
    )
    .expect("fixture collection is valid")
}

fn id(value: &str) -> RecordId {
    RecordId::new(value).expect("fixture id is valid")
}

#[tokio::test]
async fn namespaces_isolate_apps_and_forbidden_guest_paths_fail_closed() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let alpha_principal = principal("publisher.example", "key-old", "pos-alpha");
        let rotated_alpha = principal("publisher.example", "key-new", "pos-alpha");
        let beta_principal = principal("publisher.example", "key-old", "pos-beta");
        let alpha = host
            .bind(&alpha_principal, [catalog_declaration()])
            .expect("alpha binds");
        let alpha_after_rotation = host
            .bind(&rotated_alpha, [catalog_declaration()])
            .expect("rotated alpha binds");
        let beta = host
            .bind(&beta_principal, [catalog_declaration()])
            .expect("beta binds");

        assert_eq!(alpha.namespace(), alpha_after_rotation.namespace());
        assert_ne!(alpha.namespace(), beta.namespace());
        alpha
            .create(
                "catalog",
                id("coffee"),
                json!({
                    "name": "Coffee",
                    "price_cents": 350,
                    "available": true
                }),
            )
            .await
            .expect("alpha creates its record");

        assert!(
            beta.select("catalog", id("coffee"))
                .await
                .expect("beta reads only its partition")
                .is_none()
        );
        assert_eq!(
            beta.authorize_namespace(alpha.namespace())
                .expect_err("cross-namespace attribution is denied")
                .code(),
            ApplicationDataErrorCode::CrossNamespaceDenied
        );

        for (operation, expected) in [
            (
                ForbiddenDataOperation::RawQuery,
                ApplicationDataErrorCode::RawQueryDenied,
            ),
            (
                ForbiddenDataOperation::NamespaceSwitch,
                ApplicationDataErrorCode::NamespaceSwitchDenied,
            ),
            (
                ForbiddenDataOperation::DatabaseSwitch,
                ApplicationDataErrorCode::DatabaseSwitchDenied,
            ),
        ] {
            assert_eq!(
                beta.execute(GuestDataRequest::Forbidden(operation))
                    .await
                    .expect_err("forbidden guest path is denied")
                    .code(),
                expected
            );
        }
    }

    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn declared_schema_supports_select_create_merge_patch_list_and_delete() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let app = principal("publisher.example", "key", "pos");
        let data = host
            .bind(&app, [catalog_declaration()])
            .expect("application binds");

        assert_eq!(
            data.create(
                "catalog",
                id("invalid"),
                json!({ "name": "Missing fields" }),
            )
            .await
            .expect_err("incomplete record is rejected")
            .code(),
            ApplicationDataErrorCode::SchemaViolation
        );

        data.create(
            "catalog",
            id("latte"),
            json!({
                "name": "Latte",
                "price_cents": 450,
                "category": "drinks",
                "available": true
            }),
        )
        .await
        .expect("record is created");

        let merged = data
            .update_merge("catalog", id("latte"), json!({ "price_cents": 475 }))
            .await
            .expect("record is merged");
        assert_eq!(merged.value["price_cents"], 475);

        let patched = data
            .update_patch(
                "catalog",
                id("latte"),
                vec![
                    PatchOperation::Set {
                        field: "available".to_owned(),
                        value: json!(false),
                    },
                    PatchOperation::Remove {
                        field: "category".to_owned(),
                    },
                ],
            )
            .await
            .expect("record is patched");
        assert_eq!(patched.value["available"], false);
        assert!(patched.value.get("category").is_none());

        let selected = data
            .select("catalog", id("latte"))
            .await
            .expect("record is selected")
            .expect("record exists");
        assert_eq!(selected, patched);
        assert_eq!(
            data.list("catalog").await.expect("collection lists"),
            vec![patched]
        );
        assert!(
            data.delete("catalog", id("latte"))
                .await
                .expect("record deletes")
        );
        assert!(
            data.list("catalog")
                .await
                .expect("collection lists")
                .is_empty()
        );
    }

    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn pos_catalog_and_cart_persist_across_store_reopen() {
    let directory = tempdir().expect("temporary directory is created");
    let app = principal("publisher.example", "key", "restaurant-pos");

    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);
    {
        let data = host
            .bind(&app, [catalog_declaration(), cart_declaration()])
            .expect("application binds");
        for (item_id, name, price) in [
            ("burger", "Burger", 1299),
            ("fries", "Fries", 499),
            ("shake", "Shake", 599),
        ] {
            data.create(
                "catalog",
                id(item_id),
                json!({
                    "name": name,
                    "price_cents": price,
                    "category": "menu",
                    "available": true
                }),
            )
            .await
            .expect("catalog item persists");
        }
        data.create(
            "cart",
            id("line-1"),
            json!({ "catalog_item_id": "burger", "quantity": 2 }),
        )
        .await
        .expect("cart line persists");
    }
    host.into_inner().close().await.expect("store closes");

    let reopened = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store reopens");
    let host = ApplicationDataHost::new(reopened);
    {
        let data = host
            .bind(&app, [catalog_declaration(), cart_declaration()])
            .expect("application rebinds");
        let records = data.list("catalog").await.expect("catalog reloads");
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["burger", "fries", "shake"]
        );
        assert_eq!(records[0].value["price_cents"], 1299);
        let cart = data.list("cart").await.expect("cart reloads");
        assert_eq!(cart.len(), 1);
        assert_eq!(cart[0].value["catalog_item_id"], "burger");
        assert_eq!(cart[0].value["quantity"], 2);
    }
    host.into_inner().close().await.expect("store closes");
}
