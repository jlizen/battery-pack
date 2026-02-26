//! Build-time documentation generation for battery packs.
//!
//! Renders Handlebars templates with battery pack metadata to produce
//! documentation for docs.rs. The rendering pipeline is split into
//! pure functions (`build_context`, `render_docs`) for testability,
//! with `generate_docs` as the I/O entry point for build.rs.

use bphelper_manifest::BatteryPackSpec;
use serde::Serialize;
use std::collections::BTreeMap;

// ============================================================================
// Error type
// ============================================================================

/// Errors from documentation generation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("template rendering failed: {0}")]
    Render(#[from] handlebars::RenderError),

    #[error("template parse error: {0}")]
    Template(#[from] Box<handlebars::TemplateError>),

    #[error("reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("cargo metadata failed: {0}")]
    Metadata(String),
}

// ============================================================================
// Template context types
// ============================================================================

/// Context for Handlebars template rendering.
// [impl docgen.vars.crates]
// [impl docgen.vars.features]
// [impl docgen.vars.readme]
// [impl docgen.vars.package]
// [impl docgen.template.custom]
#[derive(Debug, Serialize)]
pub struct DocsContext {
    /// Non-hidden curated crates.
    pub crates: Vec<CrateEntry>,
    /// Named feature groups.
    pub features: Vec<FeatureEntry>,
    /// Contents of README.md.
    pub readme: String,
    /// Package metadata.
    pub package: PackageInfo,
}

/// A single crate in the template context.
#[derive(Debug, Serialize)]
pub struct CrateEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub features: Vec<String>,
    pub dep_kind: String,
}

/// A feature group in the template context.
#[derive(Debug, Serialize)]
pub struct FeatureEntry {
    pub name: String,
    pub crates: Vec<String>,
}

/// Package-level metadata in the template context.
#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repository: String,
}

// ============================================================================
// Core rendering pipeline (pure functions)
// ============================================================================

/// Build the template context from a parsed spec, crate descriptions, and readme.
///
/// This is a pure function with no I/O — all inputs are passed in.
/// Hidden crates are excluded from the context.
// [impl docgen.hidden.excluded]
pub fn build_context(
    spec: &BatteryPackSpec,
    descriptions: &BTreeMap<String, String>,
    readme: &str,
) -> DocsContext {
    let visible = spec.visible_crates();

    let crates = visible
        .iter()
        .map(|(&name, &crate_spec)| CrateEntry {
            name: name.to_string(),
            version: crate_spec.version.clone(),
            description: descriptions.get(name).cloned().unwrap_or_default(),
            features: crate_spec.features.iter().cloned().collect(),
            dep_kind: crate_spec.dep_kind.to_string(),
        })
        .collect();

    let features = spec
        .features
        .iter()
        .map(|(name, crate_names)| FeatureEntry {
            name: name.clone(),
            crates: crate_names.iter().cloned().collect(),
        })
        .collect();

    DocsContext {
        crates,
        features,
        readme: readme.to_string(),
        package: PackageInfo {
            name: spec.name.clone(),
            version: spec.version.clone(),
            description: spec.description.clone(),
            repository: spec.repository.clone().unwrap_or_default(),
        },
    }
}

/// Render a Handlebars template string with the given context.
///
/// Registers the `{{readme}}` and `{{crate-table}}` helpers.
/// HTML escaping is disabled since we generate markdown.
// [impl docgen.template.handlebars]
// [impl docgen.helper.readme]
// [impl docgen.helper.crate-table]
pub fn render_docs(template: &str, context: &DocsContext) -> Result<String, Error> {
    let mut hbs = handlebars::Handlebars::new();
    hbs.set_strict_mode(false);
    // We generate markdown, not HTML — disable escaping.
    hbs.register_escape_fn(handlebars::no_escape);

    hbs.register_helper("readme", Box::new(ReadmeHelper));
    hbs.register_helper("crate-table", Box::new(CrateTableHelper));

    hbs.register_template_string("docs", template)
        .map_err(|e| Error::Template(Box::new(e)))?;

    Ok(hbs.render("docs", context)?)
}

// ============================================================================
// Handlebars helpers
// ============================================================================

/// Helper that expands `{{readme}}` to the readme contents from context.
struct ReadmeHelper;

impl handlebars::HelperDef for ReadmeHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _: &handlebars::Helper<'rc>,
        _: &'reg handlebars::Handlebars<'reg>,
        ctx: &'rc handlebars::Context,
        _: &mut handlebars::RenderContext<'reg, 'rc>,
        out: &mut dyn handlebars::Output,
    ) -> handlebars::HelperResult {
        if let Some(readme) = ctx.data().get("readme").and_then(|v| v.as_str()) {
            out.write(readme)?;
        }
        Ok(())
    }
}

/// Helper that expands `{{crate-table}}` to a markdown table of curated crates.
// [impl docgen.helper.crate-table]
struct CrateTableHelper;

impl handlebars::HelperDef for CrateTableHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _: &handlebars::Helper<'rc>,
        _: &'reg handlebars::Handlebars<'reg>,
        ctx: &'rc handlebars::Context,
        _: &mut handlebars::RenderContext<'reg, 'rc>,
        out: &mut dyn handlebars::Output,
    ) -> handlebars::HelperResult {
        let crates = ctx
            .data()
            .get("crates")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if crates.is_empty() {
            return Ok(());
        }

        out.write("| Crate | Version | Description |\n")?;
        out.write("|-------|---------|-------------|\n")?;

        for entry in &crates {
            let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let version = entry.get("version").and_then(|v| v.as_str()).unwrap_or("");
            let description = entry
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            out.write(&format!(
                "| [{}](https://crates.io/crates/{}) | {} | {} |\n",
                name, name, version, description
            ))?;
        }

        Ok(())
    }
}

// ============================================================================
// I/O entry point for build.rs
// ============================================================================

/// Generate documentation for a battery pack.
///
/// Call this from your battery pack's `build.rs`:
///
/// ```rust,ignore
/// fn main() {
///     bphelper_build::generate_docs().unwrap();
/// }
/// ```
///
/// Reads the battery pack's Cargo.toml, `docs.handlebars.md` template,
/// and `README.md`, then renders the template and writes `docs.md`
/// to `OUT_DIR`.
// [impl docgen.build.trigger]
// [impl docgen.build.template]
pub fn generate_docs() -> Result<(), Error> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
        Error::Metadata("CARGO_MANIFEST_DIR not set — must be called from build.rs".into())
    })?;
    let out_dir = std::env::var("OUT_DIR")
        .map_err(|_| Error::Metadata("OUT_DIR not set — must be called from build.rs".into()))?;

    let manifest_path = format!("{manifest_dir}/Cargo.toml");
    let template_path = format!("{manifest_dir}/docs.handlebars.md");
    let readme_path = format!("{manifest_dir}/README.md");

    // Parse the battery pack manifest.
    let manifest_str = std::fs::read_to_string(&manifest_path).map_err(|e| Error::Io {
        path: manifest_path.clone(),
        source: e,
    })?;
    let spec = bphelper_manifest::parse_battery_pack(&manifest_str)
        .map_err(|e| Error::Metadata(e.to_string()))?;

    // Read the template.
    let template = std::fs::read_to_string(&template_path).map_err(|e| Error::Io {
        path: template_path.clone(),
        source: e,
    })?;

    // Read README (optional — empty string if missing).
    let readme = std::fs::read_to_string(&readme_path).unwrap_or_default();

    // Fetch crate descriptions via cargo metadata.
    // [impl docgen.helper.crate-table-metadata]
    let descriptions = fetch_crate_descriptions()?;

    // Build context and render.
    let context = build_context(&spec, &descriptions, &readme);
    let output = render_docs(&template, &context)?;

    // Write output.
    let output_path = format!("{out_dir}/docs.md");
    std::fs::write(&output_path, output).map_err(|e| Error::Io {
        path: output_path,
        source: e,
    })?;

    // Set up cargo rebuild triggers.
    println!("cargo:rerun-if-changed={manifest_path}");
    println!("cargo:rerun-if-changed={template_path}");
    println!("cargo:rerun-if-changed={readme_path}");

    Ok(())
}

/// Fetch crate descriptions from cargo metadata.
// [impl docgen.helper.crate-table-metadata]
fn fetch_crate_descriptions() -> Result<BTreeMap<String, String>, Error> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .map_err(|e| Error::Metadata(e.to_string()))?;

    let mut descriptions = BTreeMap::new();
    for pkg in &metadata.packages {
        if let Some(desc) = &pkg.description {
            descriptions.insert(pkg.name.to_string(), desc.clone());
        }
    }
    Ok(descriptions)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_dir() -> std::path::PathBuf {
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

    fn parse_fixture(name: &str) -> BatteryPackSpec {
        let path = fixtures_dir().join(name).join("Cargo.toml");
        let manifest = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        bphelper_manifest::parse_battery_pack(&manifest).unwrap()
    }

    fn mock_descriptions() -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                "anyhow".into(),
                "Flexible concrete Error type built on std::error::Error".into(),
            ),
            ("thiserror".into(), "derive(Error)".into()),
            (
                "eyre".into(),
                "Flexible concrete Error type for easy idiomatic error handling".into(),
            ),
            (
                "clap".into(),
                "A simple to use, efficient, and full-featured Command Line Argument Parser".into(),
            ),
            (
                "dialoguer".into(),
                "A command line prompting library".into(),
            ),
            (
                "indicatif".into(),
                "A progress bar and CLI reporting library".into(),
            ),
            (
                "console".into(),
                "A terminal and console abstraction for Rust".into(),
            ),
            ("serde".into(), "A serialization framework".into()),
            (
                "serde_json".into(),
                "A JSON serialization file format".into(),
            ),
            ("cc".into(), "A build-time C compiler detection".into()),
            ("assert_cmd".into(), "Easy command testing".into()),
            ("predicates".into(), "Boolean predicate combinators".into()),
        ])
    }

    // ================================================================
    // build_context() tests
    // ================================================================

    #[test]
    // [verify docgen.vars.crates]
    // [verify docgen.vars.package]
    fn test_context_basic() {
        let spec = parse_fixture("basic-battery-pack");
        let descriptions = mock_descriptions();
        let ctx = build_context(&spec, &descriptions, "# Hello");

        // Package metadata
        assert_eq!(ctx.package.name, "basic-battery-pack");
        assert_eq!(ctx.package.version, "0.1.0");
        assert_eq!(ctx.package.description, "A simple test battery pack");
        assert_eq!(ctx.package.repository, "");

        // All 3 crates should be visible (no hidden config)
        let names: Vec<&str> = ctx.crates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["anyhow", "eyre", "thiserror"]);

        // Check a specific crate entry
        let anyhow = ctx.crates.iter().find(|c| c.name == "anyhow").unwrap();
        assert_eq!(anyhow.version, "1");
        assert_eq!(
            anyhow.description,
            "Flexible concrete Error type built on std::error::Error"
        );
        assert_eq!(anyhow.dep_kind, "dependencies");
    }

    #[test]
    // [verify docgen.hidden.excluded]
    fn test_context_hidden_excluded() {
        let spec = parse_fixture("fancy-battery-pack");
        let descriptions = mock_descriptions();
        let ctx = build_context(&spec, &descriptions, "");

        let names: Vec<&str> = ctx.crates.iter().map(|c| c.name.as_str()).collect();
        // serde, serde_json, cc should be hidden
        assert!(!names.contains(&"serde"), "serde should be hidden");
        assert!(
            !names.contains(&"serde_json"),
            "serde_json should be hidden"
        );
        assert!(!names.contains(&"cc"), "cc should be hidden");
        // visible crates should be present
        assert!(names.contains(&"clap"));
        assert!(names.contains(&"dialoguer"));
        assert!(names.contains(&"indicatif"));
        assert!(names.contains(&"console"));
    }

    #[test]
    // [verify docgen.vars.features]
    fn test_context_features() {
        let spec = parse_fixture("fancy-battery-pack");
        let descriptions = mock_descriptions();
        let ctx = build_context(&spec, &descriptions, "");

        let feature_names: Vec<&str> = ctx.features.iter().map(|f| f.name.as_str()).collect();
        assert!(feature_names.contains(&"default"));
        assert!(feature_names.contains(&"indicators"));
        assert!(feature_names.contains(&"fancy"));

        let indicators = ctx
            .features
            .iter()
            .find(|f| f.name == "indicators")
            .unwrap();
        assert_eq!(indicators.crates, vec!["console", "indicatif"]);
    }

    #[test]
    // [verify docgen.vars.readme]
    fn test_context_readme() {
        let spec = parse_fixture("basic-battery-pack");
        let descriptions = mock_descriptions();
        let readme = "# My Battery Pack\n\nThis is great.";
        let ctx = build_context(&spec, &descriptions, readme);

        assert_eq!(ctx.readme, readme);
    }

    #[test]
    // [verify docgen.vars.crates]
    fn test_context_dep_kinds() {
        let spec = parse_fixture("fancy-battery-pack");
        let descriptions = mock_descriptions();
        let ctx = build_context(&spec, &descriptions, "");

        // clap is in [dependencies] (optional)
        let clap = ctx.crates.iter().find(|c| c.name == "clap").unwrap();
        assert_eq!(clap.dep_kind, "dependencies");

        // assert_cmd is in [dev-dependencies]
        let assert_cmd = ctx.crates.iter().find(|c| c.name == "assert_cmd").unwrap();
        assert_eq!(assert_cmd.dep_kind, "dev-dependencies");

        // cc is hidden (build-dep), so not present — no build-dep to test directly
        // but we can verify the hidden ones are truly excluded
        assert!(ctx.crates.iter().all(|c| c.name != "cc"));
    }

    // ================================================================
    // render_docs() tests
    // ================================================================

    /// Helper to build a minimal context for render tests.
    fn simple_context() -> DocsContext {
        DocsContext {
            crates: vec![
                CrateEntry {
                    name: "anyhow".into(),
                    version: "1".into(),
                    description: "Flexible concrete Error type".into(),
                    features: vec![],
                    dep_kind: "dependencies".into(),
                },
                CrateEntry {
                    name: "thiserror".into(),
                    version: "2".into(),
                    description: "derive(Error)".into(),
                    features: vec!["std".into()],
                    dep_kind: "dependencies".into(),
                },
            ],
            features: vec![FeatureEntry {
                name: "default".into(),
                crates: vec!["anyhow".into(), "thiserror".into()],
            }],
            readme: "# My Pack\n\nA great battery pack.".into(),
            package: PackageInfo {
                name: "test-battery-pack".into(),
                version: "0.1.0".into(),
                description: "A test pack".into(),
                repository: "https://github.com/example/test".into(),
            },
        }
    }

    #[test]
    // [verify docgen.helper.readme]
    fn test_render_readme_helper() {
        let ctx = simple_context();
        let output = render_docs("{{readme}}", &ctx).unwrap();
        assert_eq!(output, "# My Pack\n\nA great battery pack.");
    }

    #[test]
    // [verify docgen.helper.crate-table]
    fn test_render_crate_table() {
        let ctx = simple_context();
        let output = render_docs("{{crate-table}}", &ctx).unwrap();
        expect_test::expect![[r#"
            | Crate | Version | Description |
            |-------|---------|-------------|
            | [anyhow](https://crates.io/crates/anyhow) | 1 | Flexible concrete Error type |
            | [thiserror](https://crates.io/crates/thiserror) | 2 | derive(Error) |
        "#]]
        .assert_eq(&output);
    }

    #[test]
    // [verify docgen.template.default]
    fn test_render_default_template() {
        let ctx = simple_context();
        let output = render_docs("{{readme}}\n\n{{crate-table}}", &ctx).unwrap();
        expect_test::expect![[r#"
            # My Pack

            A great battery pack.

            | Crate | Version | Description |
            |-------|---------|-------------|
            | [anyhow](https://crates.io/crates/anyhow) | 1 | Flexible concrete Error type |
            | [thiserror](https://crates.io/crates/thiserror) | 2 | derive(Error) |
        "#]]
        .assert_eq(&output);
    }

    #[test]
    // [verify docgen.template.custom]
    fn test_render_custom_template() {
        let ctx = simple_context();
        let template = r#"# {{package.name}} v{{package.version}}

{{package.description}}

## Crates

{{#each crates}}
- **{{name}}** ({{version}}): {{description}}
{{/each}}"#;
        let output = render_docs(template, &ctx).unwrap();
        expect_test::expect![[r#"
            # test-battery-pack v0.1.0

            A test pack

            ## Crates

            - **anyhow** (1): Flexible concrete Error type
            - **thiserror** (2): derive(Error)
        "#]]
        .assert_eq(&output);
    }

    #[test]
    fn test_render_crate_table_empty() {
        let ctx = DocsContext {
            crates: vec![],
            features: vec![],
            readme: String::new(),
            package: PackageInfo {
                name: "empty-battery-pack".into(),
                version: "0.1.0".into(),
                description: "".into(),
                repository: "".into(),
            },
        };
        let output = render_docs("{{crate-table}}", &ctx).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    // [verify docgen.helper.crate-table]
    fn test_render_crate_table_links() {
        let ctx = simple_context();
        let output = render_docs("{{crate-table}}", &ctx).unwrap();
        assert!(output.contains("[anyhow](https://crates.io/crates/anyhow)"));
        assert!(output.contains("[thiserror](https://crates.io/crates/thiserror)"));
    }

    #[test]
    fn test_render_no_html_escaping() {
        let ctx = DocsContext {
            readme: "Use `Option<T>` and `Result<T, E>` & more".into(),
            ..simple_context()
        };
        let output = render_docs("{{readme}}", &ctx).unwrap();
        // Angle brackets and ampersand should NOT be escaped
        assert!(output.contains("<T>"));
        assert!(output.contains("<T, E>"));
        assert!(output.contains("&"));
        assert!(!output.contains("&amp;"));
        assert!(!output.contains("&lt;"));
    }

    // ================================================================
    // Full pipeline tests (parse fixture → build context → render)
    // ================================================================

    #[test]
    // [verify docgen.template.handlebars]
    fn test_full_pipeline_basic() {
        let spec = parse_fixture("basic-battery-pack");
        let descriptions = mock_descriptions();
        let readme = "# basic-battery-pack\n\nError handling crates for Rust.";
        let ctx = build_context(&spec, &descriptions, readme);
        let output = render_docs("{{readme}}\n\n{{crate-table}}", &ctx).unwrap();

        expect_test::expect![[r#"
            # basic-battery-pack

            Error handling crates for Rust.

            | Crate | Version | Description |
            |-------|---------|-------------|
            | [anyhow](https://crates.io/crates/anyhow) | 1 | Flexible concrete Error type built on std::error::Error |
            | [eyre](https://crates.io/crates/eyre) | 0.6 | Flexible concrete Error type for easy idiomatic error handling |
            | [thiserror](https://crates.io/crates/thiserror) | 2 | derive(Error) |
        "#]]
        .assert_eq(&output);
    }

    #[test]
    // [verify docgen.template.handlebars]
    // [verify docgen.hidden.excluded]
    fn test_full_pipeline_fancy() {
        let spec = parse_fixture("fancy-battery-pack");
        let descriptions = mock_descriptions();
        let readme = "# fancy-battery-pack\n\nCLI tools.";
        let ctx = build_context(&spec, &descriptions, readme);
        let output = render_docs("{{readme}}\n\n{{crate-table}}", &ctx).unwrap();

        // Hidden crates (serde*, cc) must not appear in the table
        assert!(!output.contains("serde"));
        assert!(!output.contains("| cc"));
        // Visible crates must appear
        assert!(output.contains("[clap]"));
        assert!(output.contains("[dialoguer]"));
        assert!(output.contains("[indicatif]"));
        assert!(output.contains("[console]"));
        // Dev-deps should also appear
        assert!(output.contains("[assert_cmd]"));
        assert!(output.contains("[predicates]"));
    }
}
