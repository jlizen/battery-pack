#![doc = include_str!(concat!(env!("OUT_DIR"), "/docs.md"))]

#[cfg(test)]
mod tests {
    use ::battery_pack::testing::PreviewBuilder;

    #[test]
    fn validate_templates() {
        ::battery_pack::testing::validate_templates(env!("CARGO_MANIFEST_DIR")).unwrap();
    }

    use snapbox::{assert_data_eq, file, str};

    fn file_list(defines: &[(&str, &str)]) -> String {
        let files = render(defines);
        let mut paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        paths.join("\n")
    }

    fn get_content(defines: &[(&str, &str)], path: &str) -> String {
        let files = render(defines);
        files
            .into_iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("file not found: {path}"))
            .content
    }

    fn render(defines: &[(&str, &str)]) -> Vec<::battery_pack::testing::PreviewFile> {
        let mut builder = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner");
        for (k, v) in defines {
            builder = builder.define(*k, *v);
        }
        builder.preview().unwrap()
    }

    /// Render a template and concatenate all files into a single string with
    /// `--- path ---` headers, for snapshot comparison.
    fn merged_snapshot(defines: &[(&str, &str)]) -> String {
        let mut files = render(defines);
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let mut out = String::new();
        for file in &files {
            out.push_str(&format!("--- {} ---\n", file.path));
            let trimmed = file.content.trim_end();
            out.push_str(trimmed);
            out.push_str(&format!("\n--- end {} ---\n\n", file.path));
        }
        out
    }

    fn standalone_merged_snapshot(template: &str) -> String {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template(format!("templates/{template}"))
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .preview()
            .unwrap();

        let mut sorted: Vec<_> = files.iter().collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));

        let mut out = String::new();
        for file in &sorted {
            out.push_str(&format!("--- {} ---\n", file.path));
            let trimmed = file.content.trim_end();
            out.push_str(trimmed);
            out.push_str(&format!("\n--- end {} ---\n\n", file.path));
        }
        out
    }

    #[test]
    fn minimalist_file_list() {
        assert_data_eq!(
            file_list(&[]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
deny.toml
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn maximalist_file_list() {
        assert_data_eq!(
            file_list(&[("all", "true")]),
            str![[r#"
.cargo/config.toml
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/build-binaries.yml
.github/workflows/ci.yml
.github/workflows/fuzz-nightly.yml
.github/workflows/mdbook.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
.github/workflows/stress-test.yml
Cargo.toml
README.md
_typos.toml
benches/example_bench.rs
book.toml
deny.toml
fuzz/Cargo.toml
fuzz/fuzz_targets/fuzz_example.rs
md/SUMMARY.md
md/intro.md
release-plz.toml
src/lib.rs
src/main.rs
xtask/Cargo.toml
xtask/src/main.rs
"#]]
        );
    }

    #[test]
    fn fuzzing_only_file_list() {
        assert_data_eq!(
            file_list(&[("fuzzing", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/fuzz-nightly.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
deny.toml
fuzz/Cargo.toml
fuzz/fuzz_targets/fuzz_example.rs
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn mixed_features_file_list() {
        assert_data_eq!(
            file_list(&[("fuzzing", "true"), ("spellcheck", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/fuzz-nightly.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
_typos.toml
deny.toml
fuzz/Cargo.toml
fuzz/fuzz_targets/fuzz_example.rs
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn benchmarks_only_file_list() {
        assert_data_eq!(
            file_list(&[("benchmarks", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
benches/example_bench.rs
deny.toml
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn stress_tests_only_file_list() {
        assert_data_eq!(
            file_list(&[("stress_tests", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
.github/workflows/stress-test.yml
Cargo.toml
README.md
deny.toml
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn mdbook_only_file_list() {
        assert_data_eq!(
            file_list(&[("mdbook", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/mdbook.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
book.toml
deny.toml
md/SUMMARY.md
md/intro.md
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn spellcheck_only_file_list() {
        assert_data_eq!(
            file_list(&[("spellcheck", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
_typos.toml
deny.toml
release-plz.toml
src/lib.rs
"#]]
        );
    }

    #[test]
    fn xtask_only_file_list() {
        assert_data_eq!(
            file_list(&[("xtask", "true")]),
            str![[r#"
.cargo/config.toml
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
deny.toml
release-plz.toml
src/lib.rs
xtask/Cargo.toml
xtask/src/main.rs
"#]]
        );
    }

    #[test]
    fn all_flag_matches_maximalist() {
        // -d all should produce the same files as enabling every flag individually
        let all_files = file_list(&[("all", "true")]);
        let individual_files = file_list(&[
            ("trusted_publishing", "true"),
            ("benchmarks", "true"),
            ("fuzzing", "true"),
            ("stress_tests", "true"),
            ("mdbook", "true"),
            ("spellcheck", "true"),
            ("xtask", "true"),
            ("binary_release", "true"),
            ("mutation_testing", "true"),
            ("clippy_sarif", "true"),
        ]);
        assert_eq!(all_files, individual_files);
    }

    #[test]
    fn trusted_publishing_disabled_strips_release_files() {
        let list = file_list(&[("trusted_publishing", "false")]);
        assert!(
            !list.contains("release.yml"),
            "release workflow should be stripped"
        );
        assert!(
            !list.contains("release-plz.toml"),
            "release-plz.toml should be stripped"
        );
        // Core CI should still be present
        assert!(list.contains("ci.yml"));
        assert!(list.contains("deny.toml"));
    }

    #[test]
    fn none_platform_strips_github_keeps_configs() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "none")
            .define("repo_owner", "test-owner")
            .define("all", "true")
            .preview()
            .unwrap();
        let mut paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        let list = paths.join("\n");
        // No .github/ files should be present
        assert!(
            !list.contains(".github/"),
            "ci_platform=none should strip all .github/ files"
        );
        assert_data_eq!(
            list,
            str![[r#"
.cargo/config.toml
Cargo.toml
README.md
_typos.toml
benches/example_bench.rs
book.toml
deny.toml
fuzz/Cargo.toml
fuzz/fuzz_targets/fuzz_example.rs
md/SUMMARY.md
md/intro.md
release-plz.toml
src/lib.rs
src/main.rs
xtask/Cargo.toml
xtask/src/main.rs
"#]]
        );
    }

    #[test]
    fn ci_yml_contains_gate_job() {
        let content = get_content(&[], ".github/workflows/ci.yml");
        assert!(
            content.contains("ci-pass"),
            "ci.yml should have ci-pass gate job"
        );
    }

    #[test]
    fn release_yml_uses_repo_owner() {
        let content = get_content(&[], ".github/workflows/release.yml");
        assert!(
            content.contains("test-owner"),
            "release.yml should contain repo_owner"
        );
    }

    #[test]
    fn cargo_toml_has_valid_rust_version() {
        let content = get_content(&[], "Cargo.toml");
        let line = content
            .lines()
            .find(|l| l.starts_with("rust-version"))
            .unwrap();
        // Should be like: rust-version = "1.91.1"
        let version = line.split('"').nth(1).unwrap();
        let parts: Vec<&str> = version.split('.').collect();
        assert!(parts.len() >= 2, "rust-version should be semver: {version}");
        assert!(
            parts[0].parse::<u32>().unwrap() >= 1,
            "major >= 1: {version}"
        );
    }

    #[test]
    fn fuzz_nightly_workflow_has_crash_artifacts() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .define("fuzzing", "true")
            .define("description", "")
            .preview()
            .unwrap();
        let content = files
            .iter()
            .find(|f| f.path == ".github/workflows/fuzz-nightly.yml")
            .unwrap();
        assert!(
            content.content.contains("upload-artifact"),
            "fuzz-nightly should upload crash artifacts"
        );
        assert!(
            content.content.contains("max_total_time"),
            "fuzz-nightly should have duration config"
        );
    }

    #[test]
    fn stress_test_workflow_has_timeout() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .define("stress_tests", "true")
            .define("description", "")
            .preview()
            .unwrap();
        let content = files
            .iter()
            .find(|f| f.path == ".github/workflows/stress-test.yml")
            .unwrap();
        assert!(
            content.content.contains("timeout-minutes"),
            "stress-test should have timeout"
        );
        assert!(
            content.content.contains("stress-duration"),
            "stress-test should use nextest stress-duration"
        );
    }

    #[test]
    fn description_placeholder_in_cargo_toml() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .define("description", "My cool project")
            .preview()
            .unwrap();
        let cargo = files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(
            cargo.content.contains("My cool project"),
            "description should appear in Cargo.toml"
        );
    }

    #[test]
    fn bp_managed_deps_resolved() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .define("all", "true")
            .define("description", "")
            .preview()
            .unwrap();

        // Root Cargo.toml: criterion should be resolved, not bp-managed
        let root = files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(
            root.content.contains("criterion = {"),
            "criterion should be resolved"
        );
        assert!(
            !root.content.contains("bp-managed"),
            "bp-managed should not appear in root"
        );

        // xtask/Cargo.toml: xshell, xflags, anyhow should be resolved
        let xtask = files.iter().find(|f| f.path == "xtask/Cargo.toml").unwrap();
        assert!(
            !xtask.content.contains("bp-managed"),
            "bp-managed should not appear in xtask"
        );
        assert!(xtask.content.contains("xshell"), "xshell should be present");

        // fuzz/Cargo.toml: libfuzzer-sys, arbitrary should be resolved
        let fuzz = files.iter().find(|f| f.path == "fuzz/Cargo.toml").unwrap();
        assert!(
            !fuzz.content.contains("bp-managed"),
            "bp-managed should not appear in fuzz"
        );
        assert!(
            fuzz.content.contains("libfuzzer-sys"),
            "libfuzzer-sys should be present"
        );
    }

    // -- Pin action SHA verification --

    #[test]
    fn ci_yml_has_pinned_shas() {
        let content = get_content(&[], ".github/workflows/ci.yml");
        assert!(
            !content.contains("could-not-resolve"),
            "ci.yml should not contain unresolved pin_github_action markers"
        );
        // Check that at least one line has a 40-char hex SHA after @.
        let has_sha = content.lines().any(|line| {
            let trimmed = line.trim().strip_prefix("- ").unwrap_or(line.trim());
            if let Some(rest) = trimmed.strip_prefix("uses:") {
                rest.contains('@')
                    && rest
                        .split('@')
                        .nth(1)
                        .and_then(|after| after.split_whitespace().next())
                        .is_some_and(|sha| {
                            sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
                        })
            } else {
                false
            }
        });
        assert!(has_sha, "ci.yml should contain SHA-pinned actions");
    }

    #[test]
    fn composite_action_has_pinned_shas() {
        let content = get_content(&[], ".github/actions/rust-build/action.yml");
        assert!(
            !content.contains("could-not-resolve"),
            "action.yml should not contain unresolved pin_github_action markers"
        );
    }

    // -- Standalone template tests --

    fn standalone_file_list(template: &str) -> String {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template(format!("templates/{template}"))
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .preview()
            .unwrap();
        let mut paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        paths.join("\n")
    }

    #[test]
    fn standalone_benchmarks() {
        assert_data_eq!(
            standalone_file_list("benchmarks"),
            str![[r#"
.github/workflows/benchmarks.yml
benches/example_bench.rs"#]]
        );
    }

    #[test]
    fn standalone_fuzzing() {
        assert_data_eq!(
            standalone_file_list("fuzzing"),
            str![[r#"
.github/workflows/fuzz-nightly.yml
.github/workflows/fuzz-pr.yml
fuzz/Cargo.toml
fuzz/fuzz_targets/fuzz_example.rs"#]]
        );
    }

    #[test]
    fn standalone_stress_test() {
        assert_data_eq!(
            standalone_file_list("stress-test"),
            str![[r#"
.github/workflows/stress-test.yml"#]]
        );
    }

    #[test]
    fn standalone_mdbook() {
        assert_data_eq!(
            standalone_file_list("mdbook"),
            str![[r#"
.github/workflows/mdbook.yml
book.toml
md/SUMMARY.md
md/intro.md"#]]
        );
    }

    #[test]
    fn standalone_spellcheck() {
        assert_data_eq!(
            standalone_file_list("spellcheck"),
            str![[r#"
.github/workflows/typos.yml
_typos.toml"#]]
        );
    }

    #[test]
    fn standalone_xtask() {
        assert_data_eq!(
            standalone_file_list("xtask"),
            str![[r#"
.cargo/config.toml
.github/workflows/xtask.yml
xtask/Cargo.toml
xtask/src/main.rs"#]]
        );
    }

    #[test]
    fn standalone_binary_release() {
        assert_data_eq!(
            standalone_file_list("binary-release"),
            str![[r#"
.github/workflows/build-binaries.yml
Cargo.toml
cargo-bin-section.toml
src/main.rs
"#]]
        );
    }

    #[test]
    fn binary_release_only_file_list() {
        assert_data_eq!(
            file_list(&[("binary_release", "true")]),
            str![[r#"
.github/actions/rust-build/action.yml
.github/dependabot.yml
.github/workflows/audit.yml
.github/workflows/build-binaries.yml
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/rust-next.yml
Cargo.toml
README.md
deny.toml
release-plz.toml
src/lib.rs
src/main.rs
"#]]
        );
    }

    // -- pin_github_action output verification --

    #[test]
    fn pin_github_action_subpath_in_generated_output() {
        let files = PreviewBuilder::new(env!("CARGO_MANIFEST_DIR"))
            .template("templates/full")
            .define("ci_platform", "github")
            .define("repo_owner", "test-owner")
            .define("description", "")
            .define("clippy_sarif", "true")
            .preview()
            .unwrap();
        let ci = files
            .iter()
            .find(|f| f.path == ".github/workflows/ci.yml")
            .unwrap();
        assert!(
            ci.content.contains("github/codeql-action/upload-sarif@"),
            "should contain subpath in action reference"
        );
    }

    // -- Merged snapshot tests --
    // Each test renders a template configuration and snapshots ALL rendered
    // files concatenated with `--- path ---` headers. SHAs and MSRV are
    // masked with [..] in the snapshot files.

    #[test]
    fn snapshot_minimalist() {
        assert_data_eq!(merged_snapshot(&[]), file!["snapshots/minimalist.txt"]);
    }

    #[test]
    fn snapshot_maximalist() {
        assert_data_eq!(
            merged_snapshot(&[("all", "true")]),
            file!["snapshots/maximalist.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_benchmarks() {
        assert_data_eq!(
            standalone_merged_snapshot("benchmarks"),
            file!["snapshots/standalone_benchmarks.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_fuzzing() {
        assert_data_eq!(
            standalone_merged_snapshot("fuzzing"),
            file!["snapshots/standalone_fuzzing.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_stress_test() {
        assert_data_eq!(
            standalone_merged_snapshot("stress-test"),
            file!["snapshots/standalone_stress_test.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_mdbook() {
        assert_data_eq!(
            standalone_merged_snapshot("mdbook"),
            file!["snapshots/standalone_mdbook.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_spellcheck() {
        assert_data_eq!(
            standalone_merged_snapshot("spellcheck"),
            file!["snapshots/standalone_spellcheck.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_xtask() {
        assert_data_eq!(
            standalone_merged_snapshot("xtask"),
            file!["snapshots/standalone_xtask.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_binary_release() {
        assert_data_eq!(
            standalone_merged_snapshot("binary-release"),
            file!["snapshots/standalone_binary_release.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_trusted_publishing() {
        assert_data_eq!(
            standalone_merged_snapshot("trusted-publishing"),
            file!["snapshots/standalone_trusted_publishing.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_mutation_testing() {
        assert_data_eq!(
            standalone_merged_snapshot("mutation-testing"),
            file!["snapshots/standalone_mutation_testing.txt"]
        );
    }

    #[test]
    fn snapshot_standalone_clippy_sarif() {
        assert_data_eq!(
            standalone_merged_snapshot("clippy-sarif"),
            file!["snapshots/standalone_clippy_sarif.txt"]
        );
    }
}
