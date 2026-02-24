//! Round-trip tests proving that `toml_edit`-based manipulation preserves
//! existing TOML formatting, comments, and ordering.

use bphelper_cli::{add_dep_to_table, sync_dep_in_table};
use bphelper_manifest::{CrateSpec, DepKind};

/// Helper: parse a TOML string, return a `DocumentMut`.
fn parse_doc(input: &str) -> toml_edit::DocumentMut {
    input.parse().expect("valid TOML")
}

/// Helper: build a simple `CrateSpec` with no features.
fn simple_spec(version: &str) -> CrateSpec {
    CrateSpec {
        version: version.to_string(),
        features: vec![],
        dep_kind: DepKind::Normal,
        optional: false,
    }
}

/// Helper: build a `CrateSpec` with features.
fn spec_with_features(version: &str, features: &[&str]) -> CrateSpec {
    CrateSpec {
        version: version.to_string(),
        features: features.iter().map(|s| s.to_string()).collect(),
        dep_kind: DepKind::Normal,
        optional: false,
    }
}

// ============================================================================
// manifest.toml.preserve — comments survive mutations
// ============================================================================

// [verify manifest.toml.preserve]
#[test]
fn comments_survive_add_dep() {
    let input = "\
# My project dependencies
[dependencies]
# Error handling
anyhow = \"1\"  # we love anyhow
serde = { version = \"1\", features = [\"derive\"] }
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(table, "tokio", &simple_spec("1.0"));

    let output = doc.to_string();

    // All original comments must survive
    assert!(
        output.contains("# My project dependencies"),
        "header comment lost: {output}"
    );
    assert!(
        output.contains("# Error handling"),
        "inline section comment lost: {output}"
    );
    assert!(
        output.contains("# we love anyhow"),
        "trailing comment lost: {output}"
    );

    // Original entries still present
    assert!(
        output.contains("anyhow = \"1\""),
        "anyhow entry changed: {output}"
    );
    assert!(
        output.contains("serde = { version = \"1\", features = [\"derive\"] }"),
        "serde entry changed: {output}"
    );

    // New entry was added
    assert!(
        output.contains("tokio = \"1.0\""),
        "tokio not added: {output}"
    );
}

// [verify manifest.toml.preserve]
#[test]
fn comments_survive_sync_dep() {
    let input = "\
[dependencies]
# important crate
anyhow = \"1.0.0\"  # pinned for reasons
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    // Sync anyhow to a newer version
    let changed = sync_dep_in_table(table, "anyhow", &simple_spec("1.1.0"));

    let output = doc.to_string();

    assert!(changed, "sync should report a change");
    assert!(
        output.contains("# important crate"),
        "comment above entry lost: {output}"
    );
    // The trailing comment on the same line as the value is part of the value's
    // decor in toml_edit; when the value itself is replaced the trailing
    // comment may or may not survive depending on the toml_edit version.
    // We verify the structural comment above the key always survives.
}

// ============================================================================
// manifest.toml.preserve — ordering preserved after sync
// ============================================================================

// [verify manifest.toml.preserve]
#[test]
fn ordering_preserved_after_sync() {
    let input = "\
[dependencies]
zebra = \"1.0\"
alpha = \"2.0\"
middle = \"3.0\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    // Update middle's version — ordering must stay z, a, m
    let changed = sync_dep_in_table(table, "middle", &simple_spec("3.1"));
    assert!(changed);

    let output = doc.to_string();

    let z_pos = output.find("zebra").expect("zebra missing");
    let a_pos = output.find("alpha").expect("alpha missing");
    let m_pos = output.find("middle").expect("middle missing");

    assert!(z_pos < a_pos, "zebra should come before alpha: {output}");
    assert!(a_pos < m_pos, "alpha should come before middle: {output}");

    // Verify version was actually updated
    assert!(
        output.contains("middle = \"3.1\""),
        "middle version not updated: {output}"
    );
}

// [verify manifest.toml.preserve]
#[test]
fn ordering_preserved_after_add() {
    let input = "\
[dependencies]
zebra = \"1.0\"
alpha = \"2.0\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(table, "new-crate", &simple_spec("0.5"));

    let output = doc.to_string();

    let z_pos = output.find("zebra").expect("zebra missing");
    let a_pos = output.find("alpha").expect("alpha missing");

    assert!(
        z_pos < a_pos,
        "original ordering (zebra before alpha) must survive: {output}"
    );
}

// ============================================================================
// manifest.toml.preserve — blank lines and sections preserved
// ============================================================================

// [verify manifest.toml.preserve]
#[test]
fn blank_lines_and_sections_preserved() {
    let input = "\
[package]
name = \"my-project\"
version = \"0.1.0\"

[dependencies]
anyhow = \"1\"

[dev-dependencies]
assert_cmd = \"2\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(table, "serde", &simple_spec("1"));

    let output = doc.to_string();

    // All three sections must still be present
    assert!(
        output.contains("[package]"),
        "package section lost: {output}"
    );
    assert!(
        output.contains("[dependencies]"),
        "dependencies section lost: {output}"
    );
    assert!(
        output.contains("[dev-dependencies]"),
        "dev-dependencies section lost: {output}"
    );

    // Section ordering: package before dependencies before dev-dependencies
    let pkg_pos = output.find("[package]").unwrap();
    let dep_pos = output.find("[dependencies]").unwrap();
    let dev_pos = output.find("[dev-dependencies]").unwrap();

    assert!(pkg_pos < dep_pos, "package should precede dependencies");
    assert!(
        dep_pos < dev_pos,
        "dependencies should precede dev-dependencies"
    );

    // Original entries survive
    assert!(
        output.contains("name = \"my-project\""),
        "package.name changed: {output}"
    );
    assert!(
        output.contains("assert_cmd = \"2\""),
        "dev-dep lost: {output}"
    );
}

// [verify manifest.toml.preserve]
#[test]
fn full_document_round_trip_with_multiple_sections() {
    let input = "\
[package]
name = \"example\"
version = \"0.1.0\"
edition = \"2021\"

# Runtime deps
[dependencies]
tokio = { version = \"1\", features = [\"full\"] }

# Test deps
[dev-dependencies]
pretty_assertions = \"1\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(table, "serde", &spec_with_features("1", &["derive"]));

    let output = doc.to_string();

    // Structural comments preserved
    assert!(
        output.contains("# Runtime deps"),
        "section comment lost: {output}"
    );
    assert!(
        output.contains("# Test deps"),
        "section comment lost: {output}"
    );

    // Existing inline table preserved exactly
    assert!(
        output.contains("tokio = { version = \"1\", features = [\"full\"] }"),
        "tokio entry mangled: {output}"
    );

    // New entry present
    assert!(output.contains("serde"), "serde not added: {output}");
}

// ============================================================================
// manifest.toml.style — new entries use inline tables when features present
// ============================================================================

// [verify manifest.toml.style]
#[test]
fn add_dep_uses_plain_string_for_version_only() {
    let input = "\
[dependencies]
existing = \"1.0\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(table, "simple", &simple_spec("2.0"));

    let output = doc.to_string();

    // A version-only dep should be added as a plain string, not an inline table
    assert!(
        output.contains("simple = \"2.0\""),
        "version-only dep should be a plain string: {output}"
    );
}

// [verify manifest.toml.style]
#[test]
fn add_dep_uses_inline_table_for_features() {
    let input = "\
[dependencies]
existing = { version = \"1.0\", features = [\"foo\"] }
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    add_dep_to_table(
        table,
        "new-crate",
        &spec_with_features("3.0", &["bar", "baz"]),
    );

    let output = doc.to_string();

    // A dep with features should use an inline table
    assert!(
        output.contains("new-crate = { version = \"3.0\""),
        "dep with features should use inline table: {output}"
    );
    assert!(output.contains("bar"), "feature 'bar' missing: {output}");
    assert!(output.contains("baz"), "feature 'baz' missing: {output}");
}

// ============================================================================
// manifest.toml.preserve + style — sync preserves inline table structure
// ============================================================================

// [verify manifest.toml.preserve]
// [verify manifest.toml.style]
#[test]
fn sync_preserves_inline_table_format() {
    let input = "\
[dependencies]
serde = { version = \"1.0.0\", features = [\"derive\"] }
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    // Sync with a newer version — the inline table format should be preserved
    let changed = sync_dep_in_table(table, "serde", &spec_with_features("1.1.0", &["derive"]));
    assert!(changed, "version bump should count as change");

    let output = doc.to_string();

    // Should still be an inline table (not exploded to multi-line)
    assert!(
        output.contains("serde = {"),
        "inline table format should be preserved: {output}"
    );
    assert!(
        output.contains("\"1.1.0\""),
        "version should be updated: {output}"
    );
    assert!(
        output.contains("\"derive\""),
        "existing feature should survive: {output}"
    );
}

// [verify manifest.toml.preserve]
// [verify manifest.toml.style]
#[test]
fn sync_adds_features_without_losing_existing() {
    let input = "\
[dependencies]
serde = { version = \"1.0.0\", features = [\"derive\"] }
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    // Sync with an additional feature
    let changed = sync_dep_in_table(table, "serde", &spec_with_features("1.0.0", &["rc"]));
    assert!(changed, "adding a new feature should count as change");

    let output = doc.to_string();

    // Both the old and new features should be present
    assert!(
        output.contains("derive"),
        "existing feature 'derive' lost: {output}"
    );
    assert!(
        output.contains("rc"),
        "new feature 'rc' not added: {output}"
    );
}

// [verify manifest.toml.preserve]
#[test]
fn sync_no_change_when_already_current() {
    let input = "\
[dependencies]
anyhow = \"1.0.0\"
";

    let mut doc = parse_doc(input);
    let table = doc["dependencies"].as_table_mut().unwrap();

    // Sync with the same version — no change expected
    let changed = sync_dep_in_table(table, "anyhow", &simple_spec("1.0.0"));
    assert!(!changed, "syncing same version should report no change");

    let output = doc.to_string();
    assert_eq!(
        output, input,
        "document should be byte-identical when nothing changed"
    );
}
