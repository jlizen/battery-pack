//! Group 2 tests: `cargo bp add` enhancements.
//!
//! Covers:
//!   - cli.add.features          — -F/--features flag resolves named features
//!   - cli.add.features-multiple — comma-separated and repeated -F
//!   - cli.add.default-crates    — default crates when no flags given
//!   - cli.add.no-default-features — --no-default-features skips defaults
//!   - cli.add.all-features      — --all-features selects every crate
//!   - cli.add.specific-crates   — positional crate args after pack name
//!   - cli.add.target            — --target={workspace,package,default}
//!   - cli.add.unknown-crate     — error for unknown crate, valid ones proceed

use std::collections::BTreeSet;
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

fn load_fancy_spec() -> bphelper_manifest::BatteryPackSpec {
    let fixture = fixtures_dir().join("fancy-battery-pack/Cargo.toml");
    let content = std::fs::read_to_string(&fixture).unwrap();
    bphelper_manifest::parse_battery_pack(&content).unwrap()
}

fn load_basic_spec() -> bphelper_manifest::BatteryPackSpec {
    let fixture = fixtures_dir().join("basic-battery-pack/Cargo.toml");
    let content = std::fs::read_to_string(&fixture).unwrap();
    bphelper_manifest::parse_battery_pack(&content).unwrap()
}

/// Extract crate names from a ResolvedAdd, panicking if Interactive.
fn unwrap_resolved(resolved: bphelper_cli::ResolvedAdd) -> (Vec<String>, BTreeSet<String>) {
    match resolved {
        bphelper_cli::ResolvedAdd::Crates {
            active_features,
            crates,
        } => (active_features, crates.keys().cloned().collect()),
        bphelper_cli::ResolvedAdd::Interactive => {
            panic!("expected Crates, got Interactive")
        }
    }
}

// ============================================================================
// cli.add.features — -F/--features flag parsing
// ============================================================================

// [verify cli.add.features]
#[test]
fn features_long_flag_parsed() {
    use clap::Parser;
    let cli = bphelper_cli::Cli::try_parse_from([
        "cargo",
        "bp",
        "add",
        "cli",
        "--features",
        "indicators",
    ])
    .expect("--features flag should be accepted");

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { features, .. } => {
                assert_eq!(features, vec!["indicators"]);
            }
            other => panic!("expected Add, got {:?}", std::mem::discriminant(&other)),
        },
    }
}

// [verify cli.add.features]
#[test]
fn features_short_flag_parsed() {
    use clap::Parser;
    let cli = bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "-F", "indicators"])
        .expect("-F flag should be accepted");

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { features, .. } => {
                assert_eq!(features, vec!["indicators"]);
            }
            other => panic!("expected Add, got {:?}", std::mem::discriminant(&other)),
        },
    }
}

// [verify cli.add.features]
#[test]
fn features_old_with_flag_rejected() {
    use clap::Parser;
    let result =
        bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "--with", "indicators"]);
    assert!(result.is_err(), "old --with flag should be rejected");
}

// ============================================================================
// cli.add.features-multiple — comma-separated and repeated -F
// ============================================================================

// [verify cli.add.features-multiple]
#[test]
fn features_comma_separated() {
    use clap::Parser;
    let cli =
        bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "-F", "indicators,fancy"])
            .unwrap();

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { features, .. } => {
                assert_eq!(features, vec!["indicators", "fancy"]);
            }
            other => panic!("expected Add, got {:?}", std::mem::discriminant(&other)),
        },
    }
}

// [verify cli.add.features-multiple]
#[test]
fn features_repeated_flag() {
    use clap::Parser;
    let cli = bphelper_cli::Cli::try_parse_from([
        "cargo",
        "bp",
        "add",
        "cli",
        "-F",
        "indicators",
        "-F",
        "fancy",
    ])
    .unwrap();

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { features, .. } => {
                assert_eq!(features, vec!["indicators", "fancy"]);
            }
            other => panic!("expected Add, got {:?}", std::mem::discriminant(&other)),
        },
    }
}

// ============================================================================
// cli.add.default-crates — resolve_add_crates with no flags
// ============================================================================

// [verify cli.add.default-crates]
#[test]
fn resolve_default_crates_returns_interactive_when_choices_exist() {
    // When no flags are given and the pack has meaningful choices,
    // resolve_add_crates returns Interactive (the caller decides
    // whether to show the picker or fall back to defaults).
    let spec = load_basic_spec();
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "basic-battery-pack", &[], false, false, &[]);
    assert!(
        matches!(resolved, bphelper_cli::ResolvedAdd::Interactive),
        "should signal Interactive when pack has choices and no flags given"
    );
}

// [verify cli.add.default-crates]
#[test]
fn resolve_default_crates_basic_via_explicit_feature() {
    // Explicitly requesting "default" feature bypasses the interactive path
    // and resolves default crates directly.
    // basic-battery-pack: default = ["anyhow", "thiserror"], eyre is optional.
    let spec = load_basic_spec();
    let feat = vec!["default".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "basic-battery-pack", &feat, false, false, &[]);
    let (_, crate_names) = unwrap_resolved(resolved);

    assert!(crate_names.contains("anyhow"));
    assert!(crate_names.contains("thiserror"));
    assert!(
        !crate_names.contains("eyre"),
        "eyre is optional, not in default"
    );
}

// [verify cli.add.default-crates]
#[test]
fn resolve_default_crates_fancy_via_named_feature() {
    // Passing -F indicators forces the non-interactive path and includes
    // both default and indicators crates.
    // fancy-battery-pack: default = [clap, dialoguer], indicators = [indicatif, console]
    let spec = load_fancy_spec();
    let feat = vec!["indicators".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &feat, false, false, &[]);
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["default", "indicators"]);
    assert!(crate_names.contains("clap"), "default crate");
    assert!(crate_names.contains("dialoguer"), "default crate");
    assert!(crate_names.contains("indicatif"), "indicators crate");
    assert!(crate_names.contains("console"), "indicators crate");
    assert!(
        !crate_names.contains("assert_cmd"),
        "dev dep not in default or indicators"
    );
}

// ============================================================================
// cli.add.features — resolution with named features
// ============================================================================

// [verify cli.add.features]
#[test]
fn resolve_with_named_feature_adds_to_defaults() {
    // -F indicators → default + indicators crates.
    // fancy-battery-pack: default = [clap, dialoguer], indicators = [indicatif, console]
    let spec = load_fancy_spec();
    let features_flag = vec!["indicators".to_string()];
    let resolved = bphelper_cli::resolve_add_crates(
        &spec,
        "fancy-battery-pack",
        &features_flag,
        false,
        false,
        &[],
    );
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["default", "indicators"]);
    // Default crates
    assert!(crate_names.contains("clap"));
    assert!(crate_names.contains("dialoguer"));
    // indicators crates
    assert!(crate_names.contains("indicatif"));
    assert!(crate_names.contains("console"));
}

// [verify cli.add.features]
#[test]
fn resolve_with_all_errors_feature() {
    // basic-battery-pack: all-errors = [anyhow, thiserror, eyre]
    // -F all-errors → default + all-errors, which adds eyre
    let spec = load_basic_spec();
    let features_flag = vec!["all-errors".to_string()];
    let resolved = bphelper_cli::resolve_add_crates(
        &spec,
        "basic-battery-pack",
        &features_flag,
        false,
        false,
        &[],
    );
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["default", "all-errors"]);
    assert!(crate_names.contains("anyhow"));
    assert!(crate_names.contains("thiserror"));
    assert!(crate_names.contains("eyre"), "all-errors includes eyre");
}

// ============================================================================
// cli.add.no-default-features — skips defaults
// ============================================================================

// [verify cli.add.no-default-features]
#[test]
fn resolve_no_default_features_alone_yields_nothing() {
    // --no-default-features with no -F → empty feature list → no crates
    let spec = load_fancy_spec();
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &[], true, false, &[]);
    let (features, crate_names) = unwrap_resolved(resolved);

    assert!(features.is_empty(), "no features active");
    assert!(crate_names.is_empty(), "no crates resolved");
}

// [verify cli.add.no-default-features]
#[test]
fn resolve_no_default_features_with_named_feature() {
    // --no-default-features -F indicators → only indicators crates
    let spec = load_fancy_spec();
    let features_flag = vec!["indicators".to_string()];
    let resolved = bphelper_cli::resolve_add_crates(
        &spec,
        "fancy-battery-pack",
        &features_flag,
        true,
        false,
        &[],
    );
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["indicators"]);
    assert!(crate_names.contains("indicatif"));
    assert!(crate_names.contains("console"));
    assert!(!crate_names.contains("clap"), "default crate excluded");
    assert!(!crate_names.contains("dialoguer"), "default crate excluded");
}

// ============================================================================
// cli.add.all-features — resolves every crate
// ============================================================================

// [verify cli.add.all-features]
#[test]
fn resolve_all_features_fancy() {
    // --all-features on fancy-battery-pack → all visible crates
    let spec = load_fancy_spec();
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &[], false, true, &[]);
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["all"]);
    // All dependency-section crates (hidden crates are still resolved)
    assert!(crate_names.contains("clap"));
    assert!(crate_names.contains("dialoguer"));
    assert!(crate_names.contains("indicatif"));
    assert!(crate_names.contains("console"));
    // Dev and build deps too
    assert!(crate_names.contains("assert_cmd"));
    assert!(crate_names.contains("predicates"));
}

// [verify cli.add.all-features]
#[test]
fn resolve_all_features_basic() {
    // --all-features on basic-battery-pack → anyhow, thiserror, eyre
    let spec = load_basic_spec();
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "basic-battery-pack", &[], false, true, &[]);
    let (features, crate_names) = unwrap_resolved(resolved);

    assert_eq!(features, vec!["all"]);
    assert!(crate_names.contains("anyhow"));
    assert!(crate_names.contains("thiserror"));
    assert!(
        crate_names.contains("eyre"),
        "optional eyre included with --all-features"
    );
}

// ============================================================================
// cli.add.specific-crates — positional args select exact crates
// ============================================================================

// [verify cli.add.specific-crates]
#[test]
fn specific_crates_parsed() {
    use clap::Parser;
    let cli = bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "clap", "dialoguer"])
        .unwrap();

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { crates, .. } => {
                assert_eq!(crates, vec!["clap", "dialoguer"]);
            }
            other => panic!("expected Add, got {:?}", std::mem::discriminant(&other)),
        },
    }
}

// [verify cli.add.specific-crates]
#[test]
fn resolve_specific_crates_selects_only_named() {
    // Selecting "clap" from fancy-battery-pack should return only clap,
    // ignoring default feature and other crates.
    let spec = load_fancy_spec();
    let specific = vec!["clap".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &[], false, false, &specific);
    let (features, crate_names) = unwrap_resolved(resolved);

    assert!(
        features.is_empty(),
        "specific crates mode records no features"
    );
    assert_eq!(crate_names.len(), 1);
    assert!(crate_names.contains("clap"));
}

// [verify cli.add.specific-crates]
#[test]
fn resolve_specific_crates_multiple() {
    // Selecting "anyhow" and "eyre" from basic-battery-pack
    let spec = load_basic_spec();
    let specific = vec!["anyhow".to_string(), "eyre".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "basic-battery-pack", &[], false, false, &specific);
    let (_, crate_names) = unwrap_resolved(resolved);

    assert_eq!(crate_names.len(), 2);
    assert!(crate_names.contains("anyhow"));
    assert!(crate_names.contains("eyre"));
    assert!(!crate_names.contains("thiserror"), "not requested");
}

// [verify cli.add.specific-crates]
#[test]
fn resolve_specific_crates_ignores_features_flag() {
    // When specific crates are given, -F flags are irrelevant — only the
    // named crates matter. (The features flag is still parsed by clap but
    // specific_crates takes priority in resolve_add_crates.)
    let spec = load_fancy_spec();
    let features_flag = vec!["indicators".to_string()];
    let specific = vec!["dialoguer".to_string()];
    let resolved = bphelper_cli::resolve_add_crates(
        &spec,
        "fancy-battery-pack",
        &features_flag,
        false,
        false,
        &specific,
    );
    let (_, crate_names) = unwrap_resolved(resolved);

    assert_eq!(crate_names.len(), 1);
    assert!(crate_names.contains("dialoguer"));
    assert!(
        !crate_names.contains("indicatif"),
        "not selected despite -F indicators"
    );
}

// ============================================================================
// cli.add.unknown-crate — errors for unknown, valid ones proceed
// ============================================================================

// [verify cli.add.unknown-crate]
#[test]
fn resolve_unknown_crate_skipped_valid_proceeds() {
    // "nonexistent" is unknown, "clap" is valid → only clap returned
    let spec = load_fancy_spec();
    let specific = vec!["nonexistent".to_string(), "clap".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &[], false, false, &specific);
    let (_, crate_names) = unwrap_resolved(resolved);

    assert_eq!(crate_names.len(), 1);
    assert!(crate_names.contains("clap"));
    assert!(!crate_names.contains("nonexistent"));
}

// [verify cli.add.unknown-crate]
#[test]
fn resolve_all_unknown_crates_yields_empty() {
    let spec = load_fancy_spec();
    let specific = vec!["nope".to_string(), "also-nope".to_string()];
    let resolved =
        bphelper_cli::resolve_add_crates(&spec, "fancy-battery-pack", &[], false, false, &specific);
    let (_, crate_names) = unwrap_resolved(resolved);

    assert!(crate_names.is_empty());
}

// ============================================================================
// cli.add.target — flag parsing
// ============================================================================

// [verify cli.add.target]
#[test]
fn target_values_parsed() {
    use clap::Parser;

    for (arg, expected) in [
        ("workspace", bphelper_cli::AddTarget::Workspace),
        ("package", bphelper_cli::AddTarget::Package),
        ("default", bphelper_cli::AddTarget::Default),
    ] {
        let cli = bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "--target", arg])
            .unwrap_or_else(|e| panic!("--target {arg} should parse: {e}"));

        match cli.command {
            bphelper_cli::Commands::Bp { command, .. } => match command {
                bphelper_cli::BpCommands::Add { target, .. } => {
                    assert_eq!(target, Some(expected), "for --target {arg}");
                }
                _ => panic!("expected Add"),
            },
        }
    }
}

// [verify cli.add.target]
#[test]
fn target_omitted_is_none() {
    use clap::Parser;
    let cli = bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli"]).unwrap();

    match cli.command {
        bphelper_cli::Commands::Bp { command, .. } => match command {
            bphelper_cli::BpCommands::Add { target, .. } => {
                assert!(target.is_none());
            }
            _ => panic!("expected Add"),
        },
    }
}

// [verify cli.add.target]
#[test]
fn target_invalid_value_rejected() {
    use clap::Parser;
    let result =
        bphelper_cli::Cli::try_parse_from(["cargo", "bp", "add", "cli", "--target", "invalid"]);
    assert!(result.is_err());
}
