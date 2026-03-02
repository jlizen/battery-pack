//! Integration tests for `cargo bp show`.

use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
}

// [verify cli.show.hidden]
#[test]
fn show_detail_excludes_hidden_crates() {
    let fancy_path = fixtures_dir().join("fancy-battery-pack");
    let detail =
        bphelper_cli::fetch_battery_pack_detail("fancy", Some(fancy_path.to_str().unwrap()))
            .unwrap();

    // hidden = ["serde*", "cc"] in the fancy fixture
    // Glob-matched deps excluded
    assert!(!detail.crates.contains(&"serde".to_string()));
    assert!(!detail.crates.contains(&"serde_json".to_string()));
    // Exact-matched build-dep excluded
    assert!(!detail.crates.contains(&"cc".to_string()));

    // Visible crates still present
    assert!(detail.crates.contains(&"clap".to_string()));
    assert!(detail.crates.contains(&"dialoguer".to_string()));
    assert!(detail.crates.contains(&"indicatif".to_string()));
    assert!(detail.crates.contains(&"console".to_string()));
    assert!(detail.crates.contains(&"assert_cmd".to_string()));
    assert!(detail.crates.contains(&"predicates".to_string()));
}

// [verify cli.show.hidden]
#[test]
fn show_detail_no_hidden_returns_all_crates() {
    let basic_path = fixtures_dir().join("basic-battery-pack");
    let detail =
        bphelper_cli::fetch_battery_pack_detail("basic", Some(basic_path.to_str().unwrap()))
            .unwrap();

    // basic fixture has no hidden config — all crates should appear
    assert!(detail.crates.contains(&"anyhow".to_string()));
    assert!(detail.crates.contains(&"thiserror".to_string()));
    assert!(detail.crates.contains(&"eyre".to_string()));
    assert_eq!(detail.crates.len(), 3);
}
