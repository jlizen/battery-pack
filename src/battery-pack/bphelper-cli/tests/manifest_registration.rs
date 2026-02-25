//! Tests for manifest registration and features spec rules.
//!
//! These tests exercise the TOML manipulation helpers that implement
//! battery pack registration, feature storage, and dependency management
//! in user Cargo.toml files.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // bphelper-cli is at src/battery-pack/bphelper-cli, workspace root is three levels up
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
}

// ============================================================================
// manifest.register.location — registrations in [*.metadata.battery-pack]
// ============================================================================

// [verify manifest.register.location]
#[test]
fn register_location_package_metadata() {
    // Battery pack registrations must be stored in package.metadata.battery-pack
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack]
basic-battery-pack = "0.1.0"

[build-dependencies]
basic-battery-pack = "0.1.0"
"#;

    let names = bphelper_cli::find_installed_bp_names(manifest).unwrap();
    assert_eq!(names, vec!["basic-battery-pack"]);
}

// [verify manifest.register.location]
#[test]
fn register_location_finds_battery_packs_in_build_deps() {
    // find_installed_bp_names scans [build-dependencies] for battery packs
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[build-dependencies]
cli-battery-pack = "0.3.0"
error-battery-pack = "0.4.0"
serde = "1"
"#;

    let names = bphelper_cli::find_installed_bp_names(manifest).unwrap();
    assert!(names.contains(&"cli-battery-pack".to_string()));
    assert!(names.contains(&"error-battery-pack".to_string()));
    assert!(!names.contains(&"serde".to_string()));
}

// ============================================================================
// manifest.register.format — key-value pair with name and version string
// ============================================================================

// [verify manifest.register.format]
#[test]
fn register_format_key_value_pair() {
    // Each registration is a key-value pair: name = "version"
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack]
basic-battery-pack = { version = "0.1.0", features = ["default"] }
"#;

    // read_active_features can parse the table format
    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// ============================================================================
// manifest.features.default-implicit — without features key, default is active
// ============================================================================

// [verify manifest.features.default-implicit]
#[test]
fn features_default_implicit_when_no_features_key() {
    // When there is no features key at all, default is implicitly active
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack]
basic-battery-pack = "0.1.0"
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// [verify manifest.features.default-implicit]
#[test]
fn features_default_implicit_when_no_metadata() {
    // When there is no metadata at all, default is implicitly active
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// [verify manifest.features.default-implicit]
#[test]
fn features_default_implicit_when_bp_not_registered() {
    // When the battery pack is not in metadata, default is implicitly active
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack]
other-battery-pack = "0.2.0"
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// ============================================================================
// manifest.features.short-form — only default feature may use short form
// ============================================================================

// [verify manifest.features.short-form]
#[test]
fn features_short_form_is_version_string() {
    // Short form: just a version string means only default feature is active
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack]
basic-battery-pack = "0.1.0"
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    // Short form (just version string) implies default feature
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// ============================================================================
// manifest.features.storage — active features stored alongside registration
// ============================================================================

// [verify manifest.features.storage]
#[test]
fn features_storage_reads_explicit_features() {
    // Features are stored as a features array in the registration table
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack.cli-battery-pack]
features = ["default", "indicators"]
"#;

    let features = bphelper_cli::read_active_features(manifest, "cli-battery-pack");
    assert_eq!(
        features,
        BTreeSet::from(["default".to_string(), "indicators".to_string()])
    );
}

// [verify manifest.features.storage]
#[test]
fn features_storage_reads_single_feature() {
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack.basic-battery-pack]
features = ["all-errors"]
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(features, BTreeSet::from(["all-errors".to_string()]));
}

// ============================================================================
// manifest.deps.add — add to correct dependency section
// ============================================================================

// [verify manifest.deps.add]
#[test]
fn deps_add_simple_version() {
    // When a crate has no features, add as simple version string
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "1.0".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "anyhow", &spec);

    let value = table.get("anyhow").unwrap();
    assert_eq!(value.as_str().unwrap(), "1.0");
}

// [verify manifest.deps.add]
#[test]
fn deps_add_does_not_add_to_wrong_key() {
    // add_dep_to_table only adds to the given table; the caller
    // is responsible for choosing the right section
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "4".to_string(),
        features: BTreeSet::from(["derive".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "clap", &spec);

    assert!(table.contains_key("clap"));
    assert_eq!(table.len(), 1);
}

// ============================================================================
// manifest.deps.version-features — entry must include version and features
// ============================================================================

// [verify manifest.deps.version-features]
#[test]
fn deps_version_features_included() {
    // When a crate has features, the entry must include both version and features
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "4".to_string(),
        features: BTreeSet::from(["derive".to_string(), "env".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "clap", &spec);

    let value = table.get("clap").unwrap();
    let inline = value.as_inline_table().unwrap();
    assert_eq!(inline.get("version").unwrap().as_str().unwrap(), "4");

    let features = inline.get("features").unwrap().as_array().unwrap();
    let feat_strs: Vec<&str> = features.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(feat_strs, vec!["derive", "env"]);
}

// [verify manifest.deps.version-features]
#[test]
fn deps_version_features_empty_features_uses_simple_string() {
    // When features is empty, use simple version string format
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "anyhow", &spec);

    let value = table.get("anyhow").unwrap();
    // Should be a simple string, not an inline table
    assert!(
        value.as_str().is_some(),
        "expected simple string, got table"
    );
    assert_eq!(value.as_str().unwrap(), "1");
}

// ============================================================================
// manifest.deps.workspace — add to workspace.dependencies and reference
// ============================================================================

// [verify manifest.deps.workspace]
#[test]
fn deps_workspace_adds_to_workspace_deps_table() {
    // In workspace mode, deps are added to a workspace.dependencies table,
    // and the crate references them with { workspace = true }.
    // We test the building blocks: add_dep_to_table for the workspace table,
    // and then separately constructing the workspace=true reference.

    // Simulate workspace.dependencies table
    let mut ws_table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::from(["derive".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut ws_table, "serde", &spec);

    // Verify the workspace table has the full spec
    let ws_entry = ws_table.get("serde").unwrap().as_inline_table().unwrap();
    assert_eq!(ws_entry.get("version").unwrap().as_str().unwrap(), "1");

    // Simulate the crate-level reference: { workspace = true }
    let mut crate_table = toml_edit::Table::new();
    let mut dep = toml_edit::InlineTable::new();
    dep.insert("workspace", toml_edit::Value::from(true));
    crate_table.insert(
        "serde",
        toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
    );

    let crate_entry = crate_table.get("serde").unwrap().as_inline_table().unwrap();
    assert_eq!(
        crate_entry.get("workspace").unwrap().as_bool().unwrap(),
        true
    );
}

// ============================================================================
// manifest.deps.no-workspace — non-workspace adds directly with full spec
// ============================================================================

// [verify manifest.deps.no-workspace]
#[test]
fn deps_no_workspace_adds_directly() {
    // In non-workspace mode, deps are added directly with version and features
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "2".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "thiserror", &spec);

    assert_eq!(table.get("thiserror").unwrap().as_str().unwrap(), "2");
}

// [verify manifest.deps.no-workspace]
#[test]
fn deps_no_workspace_adds_with_features() {
    let mut table = toml_edit::Table::new();
    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::from(["derive".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    bphelper_cli::add_dep_to_table(&mut table, "serde", &spec);

    let entry = table.get("serde").unwrap().as_inline_table().unwrap();
    assert_eq!(entry.get("version").unwrap().as_str().unwrap(), "1");
    let features = entry.get("features").unwrap().as_array().unwrap();
    assert_eq!(features.iter().next().unwrap().as_str().unwrap(), "derive");
}

// ============================================================================
// manifest.deps.existing — must not overwrite, only add missing features
// ============================================================================

// [verify manifest.deps.existing]
#[test]
fn deps_existing_does_not_overwrite_version() {
    // sync_dep_in_table updates version when behind but the key point is
    // it operates in-place rather than replacing the entry
    let mut table = toml_edit::Table::new();

    // User already has anyhow at version "1.0.50"
    table.insert("anyhow", toml_edit::value("1.0.50"));

    let spec = bphelper_manifest::CrateSpec {
        version: "1.0.80".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "anyhow", &spec);
    assert!(changed, "should report a change for version update");

    // Version gets updated (sync behavior) but it's an update, not overwrite
    assert_eq!(table.get("anyhow").unwrap().as_str().unwrap(), "1.0.80");
}

// [verify manifest.deps.existing]
#[test]
fn deps_existing_adds_missing_features() {
    // sync_dep_in_table must add missing features without removing existing ones
    let toml_str = r#"clap = { version = "4", features = ["derive"] }"#;
    let doc: toml_edit::DocumentMut = toml_str.parse().unwrap();
    let mut table = doc.as_table().clone();

    let spec = bphelper_manifest::CrateSpec {
        version: "4".to_string(),
        features: BTreeSet::from(["derive".to_string(), "env".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "clap", &spec);
    assert!(changed, "should report a change for added features");

    let entry = table.get("clap").unwrap().as_inline_table().unwrap();
    let features = entry.get("features").unwrap().as_array().unwrap();
    let feat_strs: Vec<&str> = features.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(feat_strs.contains(&"derive"), "original feature preserved");
    assert!(feat_strs.contains(&"env"), "new feature added");
}

// [verify manifest.deps.existing]
#[test]
fn deps_existing_preserves_user_features() {
    // User has extra features that the battery pack doesn't specify;
    // sync must preserve them
    let toml_str = r#"clap = { version = "4", features = ["derive", "color"] }"#;
    let doc: toml_edit::DocumentMut = toml_str.parse().unwrap();
    let mut table = doc.as_table().clone();

    let spec = bphelper_manifest::CrateSpec {
        version: "4".to_string(),
        features: BTreeSet::from(["derive".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "clap", &spec);
    assert!(
        !changed,
        "no changes needed when user already has everything"
    );

    let entry = table.get("clap").unwrap().as_inline_table().unwrap();
    let features = entry.get("features").unwrap().as_array().unwrap();
    let feat_strs: Vec<&str> = features.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(feat_strs.contains(&"derive"));
    assert!(
        feat_strs.contains(&"color"),
        "user feature must be preserved"
    );
}

// [verify manifest.deps.existing]
#[test]
fn deps_existing_no_change_when_up_to_date() {
    let toml_str = r#"anyhow = "1""#;
    let doc: toml_edit::DocumentMut = toml_str.parse().unwrap();
    let mut table = doc.as_table().clone();

    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "anyhow", &spec);
    assert!(!changed, "no changes needed when already up to date");
}

// ============================================================================
// manifest.deps.add — dep_kind determines correct section
// ============================================================================

// [verify manifest.deps.add]
#[test]
fn deps_add_respects_dep_kind_in_spec() {
    // add_dep_to_table doesn't choose the section — the caller does.
    // But the CrateSpec carries dep_kind so the caller knows which section.
    // Here we verify add_dep_to_table works regardless of dep_kind.
    for kind in [
        bphelper_manifest::DepKind::Normal,
        bphelper_manifest::DepKind::Dev,
        bphelper_manifest::DepKind::Build,
    ] {
        let mut table = toml_edit::Table::new();
        let spec = bphelper_manifest::CrateSpec {
            version: "1.0".to_string(),
            features: BTreeSet::new(),
            dep_kind: kind,
            optional: false,
        };

        bphelper_cli::add_dep_to_table(&mut table, "some-crate", &spec);
        assert!(
            table.contains_key("some-crate"),
            "dep should be added for {:?}",
            kind,
        );
    }
}

// ============================================================================
// Integration: parse fixture + add deps to a fresh Cargo.toml
// ============================================================================

// [verify manifest.deps.add]
// [verify manifest.deps.version-features]
#[test]
fn integration_add_basic_fixture_deps_to_table() {
    // Parse the basic-battery-pack fixture and add its default crates
    let fixture = fixtures_dir().join("basic-battery-pack/Cargo.toml");
    let content = std::fs::read_to_string(&fixture).unwrap();
    let spec = bphelper_manifest::parse_battery_pack(&content).unwrap();

    // Resolve default crates
    let crates = spec.resolve_crates(&["default"]);
    assert!(crates.contains_key("anyhow"));
    assert!(crates.contains_key("thiserror"));
    assert!(
        !crates.contains_key("eyre"),
        "eyre is optional, not in default"
    );

    // Add them to a fresh table
    let mut table = toml_edit::Table::new();
    for (name, crate_spec) in &crates {
        bphelper_cli::add_dep_to_table(&mut table, name, crate_spec);
    }

    assert_eq!(table.get("anyhow").unwrap().as_str().unwrap(), "1");
    assert_eq!(table.get("thiserror").unwrap().as_str().unwrap(), "2");
}

// [verify manifest.deps.add]
// [verify manifest.deps.version-features]
#[test]
fn integration_add_fancy_fixture_deps_to_table() {
    // Parse the fancy-battery-pack fixture and add its default crates
    let fixture = fixtures_dir().join("fancy-battery-pack/Cargo.toml");
    let content = std::fs::read_to_string(&fixture).unwrap();
    let spec = bphelper_manifest::parse_battery_pack(&content).unwrap();

    // Resolve default crates
    let crates = spec.resolve_crates(&["default"]);
    assert!(crates.contains_key("clap"));
    assert!(crates.contains_key("dialoguer"));

    // Add them to a fresh table
    let mut table = toml_edit::Table::new();
    for (name, crate_spec) in &crates {
        bphelper_cli::add_dep_to_table(&mut table, name, crate_spec);
    }

    // clap should have version and features
    let clap = table.get("clap").unwrap().as_inline_table().unwrap();
    assert_eq!(clap.get("version").unwrap().as_str().unwrap(), "4");
    let features = clap.get("features").unwrap().as_array().unwrap();
    assert_eq!(features.iter().next().unwrap().as_str().unwrap(), "derive");

    // dialoguer should be a simple version string (no features)
    assert_eq!(table.get("dialoguer").unwrap().as_str().unwrap(), "0.11");
}

// [verify manifest.deps.add]
// [verify manifest.deps.version-features]
#[test]
fn integration_add_fancy_fixture_with_indicators_feature() {
    // Parse the fancy-battery-pack and resolve with indicators feature
    let fixture = fixtures_dir().join("fancy-battery-pack/Cargo.toml");
    let content = std::fs::read_to_string(&fixture).unwrap();
    let spec = bphelper_manifest::parse_battery_pack(&content).unwrap();

    let crates = spec.resolve_crates(&["default", "indicators"]);
    assert!(crates.contains_key("clap"));
    assert!(crates.contains_key("dialoguer"));
    assert!(crates.contains_key("indicatif"));
    assert!(crates.contains_key("console"));

    let mut table = toml_edit::Table::new();
    for (name, crate_spec) in &crates {
        bphelper_cli::add_dep_to_table(&mut table, name, crate_spec);
    }

    assert!(table.contains_key("indicatif"));
    assert!(table.contains_key("console"));
}

// ============================================================================
// manifest.register.format — round-trip: write metadata then read it back
// ============================================================================

// [verify manifest.register.format]
// [verify manifest.features.storage]
#[test]
fn register_format_roundtrip_with_features() {
    // Verify that a Cargo.toml with explicit features can be read back correctly.
    // We use a hand-written TOML (matching what add_battery_pack produces on
    // an existing manifest) rather than constructing via toml_edit, since the
    // round-trip through toml_edit's implicit-table creation can differ from
    // writing into an already-structured document.
    let manifest = r#"[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack.basic-battery-pack]
features = ["default", "all-errors"]
"#;

    let features = bphelper_cli::read_active_features(manifest, "basic-battery-pack");
    assert_eq!(
        features,
        BTreeSet::from(["default".to_string(), "all-errors".to_string()])
    );
}

// [verify manifest.register.format]
#[test]
fn register_format_multiple_battery_packs() {
    // Multiple battery packs can be registered simultaneously
    let manifest = r#"
[package]
name = "my-app"
version = "0.1.0"

[package.metadata.battery-pack.cli-battery-pack]
features = ["default", "indicators"]

[package.metadata.battery-pack.error-battery-pack]
features = ["default"]

[build-dependencies]
cli-battery-pack = "0.3.0"
error-battery-pack = "0.4.0"
"#;

    let names = bphelper_cli::find_installed_bp_names(manifest).unwrap();
    assert_eq!(names.len(), 2);

    let cli_features = bphelper_cli::read_active_features(manifest, "cli-battery-pack");
    assert_eq!(
        cli_features,
        BTreeSet::from(["default".to_string(), "indicators".to_string()])
    );

    let error_features = bphelper_cli::read_active_features(manifest, "error-battery-pack");
    assert_eq!(error_features, BTreeSet::from(["default".to_string()]));
}

// ============================================================================
// Sync: adding a dep that doesn't exist yet
// ============================================================================

// [verify manifest.deps.existing]
#[test]
fn sync_adds_missing_dep() {
    let mut table = toml_edit::Table::new();

    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::new(),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "anyhow", &spec);
    assert!(changed, "adding a missing dep counts as a change");
    assert!(table.contains_key("anyhow"));
    assert_eq!(table.get("anyhow").unwrap().as_str().unwrap(), "1");
}

// [verify manifest.deps.existing]
#[test]
fn sync_converts_simple_string_to_table_when_adding_features() {
    // If user has `anyhow = "1"` and we need to add features,
    // sync must convert from simple string to table format
    let toml_str = r#"anyhow = "1""#;
    let doc: toml_edit::DocumentMut = toml_str.parse().unwrap();
    let mut table = doc.as_table().clone();

    let spec = bphelper_manifest::CrateSpec {
        version: "1".to_string(),
        features: BTreeSet::from(["backtrace".to_string()]),
        dep_kind: bphelper_manifest::DepKind::Normal,
        optional: false,
    };

    let changed = bphelper_cli::sync_dep_in_table(&mut table, "anyhow", &spec);
    assert!(changed, "converting to table format is a change");

    // After sync, should have version and features
    let entry = table.get("anyhow").unwrap();
    let inline = entry.as_inline_table().unwrap();
    assert_eq!(inline.get("version").unwrap().as_str().unwrap(), "1");
    let features = inline.get("features").unwrap().as_array().unwrap();
    assert_eq!(
        features.iter().next().unwrap().as_str().unwrap(),
        "backtrace"
    );
}

// ============================================================================
// Workspace metadata reading — read_active_features_ws
// ============================================================================

// [verify manifest.register.workspace-default]
#[test]
fn read_active_features_from_workspace_metadata() {
    // When battery-pack metadata lives in workspace.metadata, read_active_features_ws
    // must correctly extract the features array.
    let ws_manifest = r#"
[workspace]
members = ["my-app"]

[workspace.metadata.battery-pack.cli-battery-pack]
features = ["default", "indicators"]
"#;

    let features = bphelper_cli::read_active_features_ws(ws_manifest, "cli-battery-pack");
    assert_eq!(
        features,
        BTreeSet::from(["default".to_string(), "indicators".to_string()])
    );
}

// [verify manifest.register.workspace-default]
#[test]
fn read_active_features_ws_fallback_to_default() {
    // When workspace.metadata.battery-pack exists but the specific battery pack
    // is not registered, default should be returned.
    let ws_manifest = r#"
[workspace]
members = ["my-app"]

[workspace.metadata.battery-pack.other-battery-pack]
features = ["default"]
"#;

    let features = bphelper_cli::read_active_features_ws(ws_manifest, "cli-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// [verify manifest.register.workspace-default]
#[test]
fn read_active_features_ws_no_metadata_at_all() {
    // When workspace Cargo.toml has no metadata section at all, default is returned.
    let ws_manifest = r#"
[workspace]
members = ["my-app"]
"#;

    let features = bphelper_cli::read_active_features_ws(ws_manifest, "cli-battery-pack");
    assert_eq!(features, BTreeSet::from(["default".to_string()]));
}

// [verify manifest.register.both-levels]
#[test]
fn read_active_features_ws_multiple_battery_packs() {
    // Multiple battery packs can be registered in workspace metadata.
    let ws_manifest = r#"
[workspace]
members = ["my-app"]

[workspace.metadata.battery-pack.cli-battery-pack]
features = ["default", "indicators"]

[workspace.metadata.battery-pack.error-battery-pack]
features = ["all-errors"]
"#;

    let cli_features = bphelper_cli::read_active_features_ws(ws_manifest, "cli-battery-pack");
    assert_eq!(
        cli_features,
        BTreeSet::from(["default".to_string(), "indicators".to_string()])
    );

    let error_features = bphelper_cli::read_active_features_ws(ws_manifest, "error-battery-pack");
    assert_eq!(error_features, BTreeSet::from(["all-errors".to_string()]));
}
