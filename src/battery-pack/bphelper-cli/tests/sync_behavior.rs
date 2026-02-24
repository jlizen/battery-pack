//! Tests for the sync behavior spec rules (manifest.sync.*).
//!
//! These tests exercise `sync_dep_in_table` directly on `toml_edit::Table`
//! values, verifying the four sync invariants:
//!
//!   - version-bump:      older version is upgraded
//!   - feature-add:       missing features are added
//!   - no-downgrade:      newer user version is left alone
//!   - no-feature-remove: user-added features are preserved

use bphelper_manifest::{CrateSpec, DepKind};
use toml_edit::DocumentMut;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `CrateSpec` with the given version and features (normal dep, non-optional).
fn spec(version: &str, features: &[&str]) -> CrateSpec {
    CrateSpec {
        version: version.to_string(),
        features: features.iter().map(|s| s.to_string()).collect(),
        dep_kind: DepKind::Normal,
        optional: false,
    }
}

/// Parse a TOML string and return a mutable reference to `[dependencies]`.
fn parse_deps(toml_str: &str) -> DocumentMut {
    toml_str.parse::<DocumentMut>().expect("valid TOML")
}

/// Read the version string for `dep_name` from the dependencies table.
fn read_version(doc: &DocumentMut, dep_name: &str) -> String {
    let deps = doc["dependencies"].as_table().expect("dependencies table");
    match deps.get(dep_name).expect("dep exists") {
        toml_edit::Item::Value(toml_edit::Value::String(s)) => s.value().to_string(),
        toml_edit::Item::Value(toml_edit::Value::InlineTable(t)) => t
            .get("version")
            .and_then(|v| v.as_str())
            .expect("version key")
            .to_string(),
        other => panic!("unexpected dep format: {:?}", other),
    }
}

/// Read the features array for `dep_name` from the dependencies table.
fn read_features(doc: &DocumentMut, dep_name: &str) -> Vec<String> {
    let deps = doc["dependencies"].as_table().expect("dependencies table");
    match deps.get(dep_name).expect("dep exists") {
        toml_edit::Item::Value(toml_edit::Value::InlineTable(t)) => t
            .get("features")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        toml_edit::Item::Value(toml_edit::Value::String(_)) => vec![],
        other => panic!("unexpected dep format: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// manifest.sync.version-bump
// ---------------------------------------------------------------------------

// [verify manifest.sync.version-bump]
#[test]
fn version_bump_simple_string() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = "1.0"
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.2", &[]));
    assert!(changed, "sync should report a change");
    assert_eq!(read_version(&doc, "serde"), "1.2");
}

// [verify manifest.sync.version-bump]
#[test]
fn version_bump_inline_table() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.2", &["derive"]));
    assert!(changed, "sync should report a change");
    assert_eq!(read_version(&doc, "serde"), "1.2");
}

// [verify manifest.sync.version-bump]
#[test]
fn version_bump_full_semver() {
    let mut doc = parse_deps(
        r#"
[dependencies]
tokio = { version = "1.0.0", features = ["full"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "tokio", &spec("1.38.0", &["full"]));
    assert!(changed, "sync should report a change");
    assert_eq!(read_version(&doc, "tokio"), "1.38.0");
}

// ---------------------------------------------------------------------------
// manifest.sync.feature-add
// ---------------------------------------------------------------------------

// [verify manifest.sync.feature-add]
#[test]
fn feature_add_to_existing_inline_table() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed =
        bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.0", &["derive", "serde_json"]));
    assert!(changed, "sync should report a change");
    let features = read_features(&doc, "serde");
    assert!(
        features.contains(&"derive".to_string()),
        "existing feature 'derive' should be present"
    );
    assert!(
        features.contains(&"serde_json".to_string()),
        "new feature 'serde_json' should be added"
    );
}

// [verify manifest.sync.feature-add]
#[test]
fn feature_add_converts_simple_string_to_table() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = "1.0"
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.0", &["derive"]));
    assert!(changed, "sync should report a change");
    let features = read_features(&doc, "serde");
    assert!(
        features.contains(&"derive".to_string()),
        "feature 'derive' should be added"
    );
    assert_eq!(
        read_version(&doc, "serde"),
        "1.0",
        "version should be preserved"
    );
}

// [verify manifest.sync.feature-add]
#[test]
fn no_change_when_features_already_present() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.0", &["derive"]));
    assert!(
        !changed,
        "sync should report no change when already up to date"
    );
}

// ---------------------------------------------------------------------------
// manifest.sync.no-downgrade
// ---------------------------------------------------------------------------

// [verify manifest.sync.no-downgrade]
#[test]
fn no_downgrade_simple_string() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = "2.0"
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.5", &[]));
    assert!(!changed, "sync must not downgrade");
    assert_eq!(
        read_version(&doc, "serde"),
        "2.0",
        "version must stay at 2.0"
    );
}

// [verify manifest.sync.no-downgrade]
#[test]
fn no_downgrade_inline_table() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "2.0", features = ["derive"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.5", &["derive"]));
    assert!(!changed, "sync must not downgrade");
    assert_eq!(
        read_version(&doc, "serde"),
        "2.0",
        "version must stay at 2.0"
    );
}

// [verify manifest.sync.no-downgrade]
#[test]
fn no_downgrade_full_semver() {
    let mut doc = parse_deps(
        r#"
[dependencies]
tokio = { version = "1.40.0", features = ["full"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "tokio", &spec("1.38.0", &["full"]));
    assert!(!changed, "sync must not downgrade");
    assert_eq!(
        read_version(&doc, "tokio"),
        "1.40.0",
        "version must stay at 1.40.0"
    );
}

// [verify manifest.sync.no-downgrade]
#[test]
fn no_downgrade_when_adding_features() {
    // User has a newer version but battery pack recommends an older one with
    // additional features.  The features should be added but the version must
    // NOT be downgraded.
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = "2.0"
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.5", &["derive"]));
    assert!(changed, "features should still be added");
    assert_eq!(
        read_version(&doc, "serde"),
        "2.0",
        "version must stay at 2.0 (no downgrade)"
    );
    let features = read_features(&doc, "serde");
    assert!(
        features.contains(&"derive".to_string()),
        "feature 'derive' should be added"
    );
}

// ---------------------------------------------------------------------------
// manifest.sync.no-feature-remove
// ---------------------------------------------------------------------------

// [verify manifest.sync.no-feature-remove]
#[test]
fn no_feature_remove_preserves_user_features() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive", "custom-user-feature"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    // Battery pack only knows about "derive", but user has "custom-user-feature" too
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.0", &["derive"]));
    assert!(
        !changed,
        "no changes needed — all bp features already present"
    );
    let features = read_features(&doc, "serde");
    assert!(
        features.contains(&"derive".to_string()),
        "'derive' must be present"
    );
    assert!(
        features.contains(&"custom-user-feature".to_string()),
        "user's 'custom-user-feature' must be preserved"
    );
}

// [verify manifest.sync.no-feature-remove]
#[test]
fn no_feature_remove_when_adding_new_features() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive", "custom-user-feature"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    // Battery pack wants "derive" + "serde_json"; user also has "custom-user-feature"
    let changed =
        bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.0", &["derive", "serde_json"]));
    assert!(changed, "new feature 'serde_json' should be added");
    let features = read_features(&doc, "serde");
    assert!(
        features.contains(&"derive".to_string()),
        "'derive' must be present"
    );
    assert!(
        features.contains(&"custom-user-feature".to_string()),
        "user's 'custom-user-feature' must be preserved"
    );
    assert!(
        features.contains(&"serde_json".to_string()),
        "new 'serde_json' must be added"
    );
    assert_eq!(features.len(), 3, "should have exactly 3 features");
}

// ---------------------------------------------------------------------------
// Combined scenarios
// ---------------------------------------------------------------------------

// [verify manifest.sync.version-bump]
// [verify manifest.sync.feature-add]
// [verify manifest.sync.no-feature-remove]
#[test]
fn version_bump_and_feature_add_preserves_user_features() {
    let mut doc = parse_deps(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive", "my-extra"] }
"#,
    );
    let table = doc["dependencies"].as_table_mut().unwrap();
    let changed = bphelper_cli::sync_dep_in_table(table, "serde", &spec("1.2", &["derive", "rc"]));
    assert!(changed, "both version and features changed");
    assert_eq!(read_version(&doc, "serde"), "1.2", "version should bump");
    let features = read_features(&doc, "serde");
    assert!(features.contains(&"derive".to_string()));
    assert!(
        features.contains(&"my-extra".to_string()),
        "user feature preserved"
    );
    assert!(features.contains(&"rc".to_string()), "new bp feature added");
}
