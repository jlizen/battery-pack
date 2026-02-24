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

// [verify cli.validate.clean]
// [verify cli.validate.checks]
#[test]
fn validate_basic_fixture_is_clean() {
    let fixture = fixtures_dir().join("basic-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(result.is_ok(), "basic-battery-pack should validate cleanly");
}

// [verify cli.validate.clean]
// [verify cli.validate.checks]
#[test]
fn validate_fancy_fixture_is_clean() {
    let fixture = fixtures_dir().join("fancy-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(result.is_ok(), "fancy-battery-pack should validate cleanly");
}

// [verify cli.validate.errors]
// [verify cli.validate.severity]
// [verify cli.validate.rule-id]
#[test]
fn validate_broken_fixture_fails() {
    let fixture = fixtures_dir().join("broken-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(
        result.is_err(),
        "broken-battery-pack should fail validation"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("error(s)"),
        "error message should report error count: {err}"
    );
}

// [verify cli.validate.workspace-error]
#[test]
fn validate_workspace_manifest_fails() {
    let fixture = fixtures_dir();
    // The fixtures directory itself has a workspace Cargo.toml
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(result.is_err(), "workspace manifest should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("workspace manifest"),
        "should mention workspace manifest: {err}"
    );
}

// [verify cli.validate.no-package]
#[test]
fn validate_nonexistent_path_fails() {
    let result = bphelper_cli::validate_battery_pack_cmd(Some("/nonexistent/path"));
    assert!(result.is_err(), "nonexistent path should fail");
}

// [verify cli.validate.default-path]
#[test]
fn validate_uses_path_argument() {
    // Verify --path correctly targets a specific directory rather than cwd
    let fixture = fixtures_dir().join("basic-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(
        result.is_ok(),
        "explicit --path to a valid fixture should succeed"
    );
}
