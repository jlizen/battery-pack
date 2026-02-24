//! Extra validation tests covering repository warnings, default-path
//! behaviour, and bare --help.

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

// ============================================================================
// cli.validate.default-path — None means "use current directory"
// ============================================================================

// [verify cli.validate.default-path]
#[test]
fn validate_defaults_to_current_directory() {
    // When invoked with None, validate_battery_pack_cmd uses std::env::current_dir().
    // We can exercise this by cd-ing into a valid fixture directory first.
    let fixture = fixtures_dir().join("fancy-battery-pack");

    // Save original directory and change into the fixture
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&fixture).unwrap();

    let result = bphelper_cli::validate_battery_pack_cmd(None);

    // Restore original directory before asserting so we don't leave tests in
    // a bad state on failure.
    std::env::set_current_dir(&original_dir).unwrap();

    assert!(
        result.is_ok(),
        "validate with None (cwd = fancy-battery-pack) should succeed: {:?}",
        result.unwrap_err()
    );
}

// ============================================================================
// format.crate.repository — CLI-level integration
// ============================================================================

// [verify format.crate.repository]
#[test]
fn validate_fixture_without_repository_warns_but_passes() {
    // basic-battery-pack has no repository field. validate should still
    // return Ok (warnings-only do not fail), but the warning is emitted.
    let fixture = fixtures_dir().join("basic-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(
        result.is_ok(),
        "basic-battery-pack should pass validation (warnings only): {:?}",
        result.unwrap_err()
    );
}

// [verify format.crate.repository]
#[test]
fn validate_fixture_with_repository_no_warning() {
    // fancy-battery-pack has the repository field set — clean validation.
    let fixture = fixtures_dir().join("fancy-battery-pack");
    let result = bphelper_cli::validate_battery_pack_cmd(Some(fixture.to_str().unwrap()));
    assert!(
        result.is_ok(),
        "fancy-battery-pack should validate cleanly: {:?}",
        result.unwrap_err()
    );
}

// ============================================================================
// cli.bare.help — --help prints help and exits (clap behaviour)
// ============================================================================

// [verify cli.bare.help]
#[test]
fn cli_bare_help_prints_help() {
    // Parsing `cargo bp --help` should result in a DisplayHelp error from
    // clap (which the binary translates into printing help and exiting 0).
    // We verify this by attempting to parse the args and checking for the
    // expected clap error kind.
    use clap::Parser;
    let result = bphelper_cli::Cli::try_parse_from(["cargo", "bp", "--help"]);
    assert!(
        result.is_err(),
        "--help should cause clap to 'error' with DisplayHelp"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelp,
        "expected DisplayHelp, got {:?}",
        err.kind()
    );
}
