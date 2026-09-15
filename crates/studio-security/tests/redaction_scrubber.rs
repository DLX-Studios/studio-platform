#![allow(missing_docs)]

use serde_json::json;
use studio_security::SensitiveValueFilter;

#[test]
fn scrubs_labeled_and_provider_shaped_values_without_prior_registration() {
    let filter = SensitiveValueFilter::new();
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijklmnopqrstuvwxyz";
    let stripe = ["sk_live_", "51", &"abcdefghij".repeat(2)].concat();
    let diagnostic = format!(
        "Authorization: Bearer {jwt}; client_secret='plain-but-sensitive'; provider={stripe}"
    );

    let safe = filter.sanitize(&diagnostic);
    assert!(!safe.contains(jwt));
    assert!(!safe.contains("plain-but-sensitive"));
    assert!(!safe.contains(&stripe));
    assert_eq!(safe.matches("[REDACTED]").count(), 3);
    assert_eq!(
        filter.sanitize("secret status: configured; notasecret=value"),
        "secret status: configured; notasecret=value"
    );
    assert!(filter.validate_persistence(&diagnostic).is_err());
}

#[test]
fn recursively_scrubs_key_shaped_structure_fields_and_string_leaves() {
    let filter = SensitiveValueFilter::new();
    let structure = json!({
        "request": {
            "apiKey": "unstructured provider credential",
            "headers": {"Authorization": "Bearer opaque-value"},
            "secretStatus": "configured",
            "attempt": 2
        },
        "events": [
            {"client-secret": "another opaque value"},
            "provider returned sk_test_51abcdefghijklmnopqrstuvwxyz"
        ]
    });

    let safe = filter.sanitize_json(&structure);
    assert_eq!(safe["request"]["apiKey"], "[REDACTED]");
    assert_eq!(safe["request"]["headers"]["Authorization"], "[REDACTED]");
    assert_eq!(safe["request"]["secretStatus"], "configured");
    assert_eq!(safe["request"]["attempt"], 2);
    assert_eq!(safe["events"][0]["client-secret"], "[REDACTED]");
    assert_eq!(safe["events"][1], "provider returned [REDACTED]");

    assert_eq!(
        structure["request"]["apiKey"],
        "unstructured provider credential"
    );
}
