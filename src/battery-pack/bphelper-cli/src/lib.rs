//! CLI for battery-pack: create and manage battery packs.

use anyhow::{Context, Result, bail};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use tar::Archive;

mod tui;

const CRATES_IO_API: &str = "https://crates.io/api/v1/crates";
const CRATES_IO_CDN: &str = "https://static.crates.io/crates";

// [impl cli.source.flag]
// [impl cli.source.replace]
#[derive(Debug, Clone)]
pub enum CrateSource {
    Registry,
    Local(PathBuf),
}

// [impl cli.bare.help]
#[derive(Parser)]
#[command(name = "cargo-bp")]
#[command(bin_name = "cargo")]
#[command(version, about = "Create and manage battery packs", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Battery pack commands
    Bp {
        // [impl cli.source.subcommands]
        /// Use a local workspace as the battery pack source (replaces crates.io)
        #[arg(long)]
        crate_source: Option<PathBuf>,

        #[command(subcommand)]
        command: BpCommands,
    },
}

#[derive(Subcommand)]
pub enum BpCommands {
    /// Create a new project from a battery pack template
    New {
        /// Name of the battery pack (e.g., "cli" resolves to "cli-battery-pack")
        battery_pack: String,

        /// Name for the new project (prompted interactively if not provided)
        #[arg(long, short = 'n')]
        name: Option<String>,

        /// Which template to use (defaults to first available, or prompts if multiple)
        // [impl cli.new.template-flag]
        #[arg(long, short = 't')]
        template: Option<String>,

        /// Use a local path instead of downloading from crates.io
        #[arg(long)]
        path: Option<String>,
    },

    /// Add a battery pack and sync its dependencies.
    ///
    /// Without arguments, opens an interactive TUI for managing all battery packs.
    /// With a battery pack name, adds that specific pack (with an interactive picker
    /// for choosing crates if the pack has features or many dependencies).
    Add {
        /// Name of the battery pack (e.g., "cli" resolves to "cli-battery-pack").
        /// Omit to open the interactive manager.
        battery_pack: Option<String>,

        /// Specific crates to add from the battery pack (ignores defaults/features)
        crates: Vec<String>,

        // [impl cli.add.features]
        // [impl cli.add.features-multiple]
        /// Named features to enable (comma-separated or repeated)
        #[arg(long = "features", short = 'F', value_delimiter = ',')]
        features: Vec<String>,

        // [impl cli.add.no-default-features]
        /// Skip the default crates; only add crates from named features
        #[arg(long)]
        no_default_features: bool,

        // [impl cli.add.all-features]
        /// Add every crate the battery pack offers
        #[arg(long)]
        all_features: bool,

        // [impl cli.add.target]
        /// Where to store the battery pack registration
        /// (workspace, package, or default)
        #[arg(long)]
        target: Option<AddTarget>,

        /// Use a local path instead of downloading from crates.io
        #[arg(long)]
        path: Option<String>,
    },

    /// Update dependencies from installed battery packs
    Sync,

    /// Enable a named feature from a battery pack
    Enable {
        /// Name of the feature to enable
        feature_name: String,

        /// Battery pack to search (optional — searches all installed if omitted)
        #[arg(long)]
        battery_pack: Option<String>,
    },

    /// List available battery packs on crates.io
    List {
        /// Filter by name (omit to list all battery packs)
        filter: Option<String>,

        /// Disable interactive TUI mode
        #[arg(long)]
        non_interactive: bool,
    },

    /// Show detailed information about a battery pack
    Show {
        /// Name of the battery pack (e.g., "cli" resolves to "cli-battery-pack")
        battery_pack: String,

        /// Use a local path instead of downloading from crates.io
        #[arg(long)]
        path: Option<String>,

        /// Disable interactive TUI mode
        #[arg(long)]
        non_interactive: bool,
    },

    /// Validate that the current battery pack is well-formed
    Validate {
        /// Path to the battery pack crate (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },
}

// [impl cli.add.target]
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AddTarget {
    /// Register in [workspace.metadata.battery-pack]
    Workspace,
    /// Register in [package.metadata.battery-pack]
    Package,
    /// Use workspace if a workspace root exists, otherwise package
    Default,
}

/// Main entry point for the CLI.
pub fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = std::env::current_dir().context("Failed to get current directory")?;

    match cli.command {
        Commands::Bp {
            crate_source,
            command,
        } => {
            let source = match crate_source {
                Some(path) => CrateSource::Local(path),
                None => CrateSource::Registry,
            };
            match command {
                BpCommands::New {
                    battery_pack,
                    name,
                    template,
                    path,
                } => new_from_battery_pack(&battery_pack, name, template, path),
                BpCommands::Add {
                    battery_pack,
                    crates,
                    features,
                    no_default_features,
                    all_features,
                    target,
                    path,
                } => match battery_pack {
                    Some(name) => add_battery_pack(
                        &name,
                        &features,
                        no_default_features,
                        all_features,
                        &crates,
                        target,
                        path.as_deref(),
                        &project_dir,
                    ),
                    // [impl cli.bare.tui]
                    None if std::io::stdout().is_terminal() => tui::run_add(source),
                    None => {
                        bail!(
                            "No battery pack specified. Use `cargo bp add <name>` or run interactively in a terminal."
                        )
                    }
                },
                BpCommands::Sync => sync_battery_packs(&project_dir),
                BpCommands::Enable {
                    feature_name,
                    battery_pack,
                } => enable_feature(&feature_name, battery_pack.as_deref(), &project_dir),
                BpCommands::List {
                    filter,
                    non_interactive,
                } => {
                    // [impl cli.list.interactive]
                    // [impl cli.list.non-interactive]
                    if !non_interactive && std::io::stdout().is_terminal() {
                        tui::run_list(source, filter)
                    } else {
                        // [impl cli.list.query]
                        // [impl cli.list.filter]
                        print_battery_pack_list(&source, filter.as_deref())
                    }
                }
                BpCommands::Show {
                    battery_pack,
                    path,
                    non_interactive,
                } => {
                    // [impl cli.show.interactive]
                    // [impl cli.show.non-interactive]
                    if !non_interactive && std::io::stdout().is_terminal() {
                        tui::run_show(&battery_pack, path.as_deref())
                    } else {
                        print_battery_pack_detail(&battery_pack, path.as_deref())
                    }
                }
                BpCommands::Validate { path } => validate_battery_pack_cmd(path.as_deref()),
            }
        }
    }
}

// ============================================================================
// crates.io API types
// ============================================================================

#[derive(Deserialize)]
struct CratesIoResponse {
    versions: Vec<VersionInfo>,
}

#[derive(Deserialize)]
struct VersionInfo {
    num: String,
    yanked: bool,
}

#[derive(Deserialize)]
struct SearchResponse {
    crates: Vec<SearchCrate>,
}

#[derive(Deserialize)]
struct SearchCrate {
    name: String,
    max_version: String,
    description: Option<String>,
}

// ============================================================================
// Battery pack metadata types (from Cargo.toml)
// ============================================================================

#[derive(Deserialize, Default)]
struct CargoManifest {
    package: Option<PackageSection>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize, Default)]
struct PackageSection {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    repository: Option<String>,
    metadata: Option<PackageMetadata>,
}

#[derive(Deserialize, Default)]
struct PackageMetadata {
    battery: Option<BatteryMetadata>,
}

#[derive(Deserialize, Default)]
struct BatteryMetadata {
    #[serde(default)]
    templates: BTreeMap<String, TemplateConfig>,
}

#[derive(Deserialize)]
pub struct TemplateConfig {
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
}

// ============================================================================
// crates.io owner types
// ============================================================================

#[derive(Deserialize)]
struct OwnersResponse {
    users: Vec<Owner>,
}

#[derive(Deserialize, Clone)]
struct Owner {
    login: String,
    name: Option<String>,
}

// ============================================================================
// GitHub API types
// ============================================================================

#[derive(Deserialize)]
struct GitHubTreeResponse {
    tree: Vec<GitHubTreeEntry>,
    #[serde(default)]
    #[allow(dead_code)]
    truncated: bool,
}

#[derive(Deserialize)]
struct GitHubTreeEntry {
    path: String,
}

// ============================================================================
// Shared data types (used by both TUI and text output)
// ============================================================================

/// Summary info for displaying in a list
#[derive(Clone)]
pub struct BatteryPackSummary {
    pub name: String,
    pub short_name: String,
    pub version: String,
    pub description: String,
}

/// Detailed battery pack info
#[derive(Clone)]
pub struct BatteryPackDetail {
    pub name: String,
    pub short_name: String,
    pub version: String,
    pub description: String,
    pub repository: Option<String>,
    pub owners: Vec<OwnerInfo>,
    pub crates: Vec<String>,
    pub extends: Vec<String>,
    pub templates: Vec<TemplateInfo>,
    pub examples: Vec<ExampleInfo>,
}

#[derive(Clone)]
pub struct OwnerInfo {
    pub login: String,
    pub name: Option<String>,
}

impl From<Owner> for OwnerInfo {
    fn from(o: Owner) -> Self {
        Self {
            login: o.login,
            name: o.name,
        }
    }
}

#[derive(Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    /// Full path in the repository (e.g., "src/cli-battery-pack/templates/simple")
    /// Resolved by searching the GitHub tree API
    pub repo_path: Option<String>,
}

#[derive(Clone)]
pub struct ExampleInfo {
    pub name: String,
    pub description: Option<String>,
    /// Full path in the repository (e.g., "src/cli-battery-pack/examples/mini-grep.rs")
    /// Resolved by searching the GitHub tree API
    pub repo_path: Option<String>,
}

// ============================================================================
// Implementation
// ============================================================================

// [impl cli.new.template]
// [impl cli.new.name-flag]
// [impl cli.new.name-prompt]
// [impl cli.path.flag]
fn new_from_battery_pack(
    battery_pack: &str,
    name: Option<String>,
    template: Option<String>,
    path_override: Option<String>,
) -> Result<()> {
    // If using local path, generate directly from there
    if let Some(path) = path_override {
        return generate_from_local(&path, name, template);
    }

    // Resolve the crate name (add -battery-pack suffix if needed)
    let crate_name = resolve_crate_name(battery_pack);

    // Look up the crate on crates.io and get the latest version
    let crate_info = lookup_crate(&crate_name)?;

    // Download and extract the crate to a temp directory
    let temp_dir = download_and_extract_crate(&crate_name, &crate_info.version)?;
    let crate_dir = temp_dir
        .path()
        .join(format!("{}-{}", crate_name, crate_info.version));

    // Read template metadata from the extracted Cargo.toml
    let manifest_path = crate_dir.join("Cargo.toml");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let templates = parse_template_metadata(&manifest_content, &crate_name)?;

    // Resolve which template to use
    let template_path = resolve_template(&templates, template.as_deref())?;

    // Generate the project from the extracted crate
    generate_from_path(&crate_dir, &template_path, name)
}

/// Result of resolving which crates to add from a battery pack.
pub enum ResolvedAdd {
    /// Resolved to a concrete set of crates (no interactive picker needed).
    Crates {
        active_features: BTreeSet<String>,
        crates: BTreeMap<String, bphelper_manifest::CrateSpec>,
    },
    /// The caller should show the interactive picker.
    Interactive,
}

/// Pure resolution logic for `cargo bp add` flags.
///
/// Given the battery pack spec and the CLI flags, determines which crates
/// to install. Returns `ResolvedAdd::Interactive` when the picker should
/// be shown (no explicit flags, TTY, meaningful choices).
///
/// When `specific_crates` is non-empty, unknown crate names are reported
/// to stderr and skipped; valid ones proceed.
// [impl cli.add.specific-crates]
// [impl cli.add.unknown-crate]
// [impl cli.add.default-crates]
// [impl cli.add.features]
// [impl cli.add.no-default-features]
// [impl cli.add.all-features]
pub fn resolve_add_crates(
    bp_spec: &bphelper_manifest::BatteryPackSpec,
    bp_name: &str,
    with_features: &[String],
    no_default_features: bool,
    all_features: bool,
    specific_crates: &[String],
) -> ResolvedAdd {
    if !specific_crates.is_empty() {
        // Explicit crate selection — ignores defaults and features.
        let mut selected = BTreeMap::new();
        for crate_name_arg in specific_crates {
            if let Some(spec) = bp_spec.crates.get(crate_name_arg.as_str()) {
                selected.insert(crate_name_arg.clone(), spec.clone());
            } else {
                eprintln!(
                    "error: crate '{}' not found in battery pack '{}'",
                    crate_name_arg, bp_name
                );
            }
        }
        return ResolvedAdd::Crates {
            active_features: BTreeSet::new(),
            crates: selected,
        };
    }

    if all_features {
        // [impl format.hidden.effect]
        return ResolvedAdd::Crates {
            active_features: BTreeSet::from(["all".to_string()]),
            crates: bp_spec.resolve_all_visible(),
        };
    }

    // When no explicit flags narrow the selection and the pack has
    // meaningful choices, signal that the caller may want to show
    // the interactive picker.
    if !no_default_features && with_features.is_empty() && bp_spec.has_meaningful_choices() {
        return ResolvedAdd::Interactive;
    }

    let mut features: BTreeSet<String> = if no_default_features {
        BTreeSet::new()
    } else {
        BTreeSet::from(["default".to_string()])
    };
    features.extend(with_features.iter().cloned());

    // When no features are active (--no-default-features with no -F),
    // return empty rather than calling resolve_crates(&[]) which
    // falls back to defaults.
    if features.is_empty() {
        return ResolvedAdd::Crates {
            active_features: features,
            crates: BTreeMap::new(),
        };
    }

    let str_features: Vec<&str> = features.iter().map(|s| s.as_str()).collect();
    let crates = bp_spec.resolve_crates(&str_features);
    ResolvedAdd::Crates {
        active_features: features,
        crates,
    }
}

// [impl cli.add.register]
// [impl cli.add.dep-kind]
// [impl cli.add.specific-crates]
// [impl cli.add.unknown-crate]
// [impl manifest.register.location]
// [impl manifest.register.format]
// [impl manifest.features.storage]
// [impl manifest.deps.add]
// [impl manifest.deps.version-features]
pub fn add_battery_pack(
    name: &str,
    with_features: &[String],
    no_default_features: bool,
    all_features: bool,
    specific_crates: &[String],
    target: Option<AddTarget>,
    path: Option<&str>,
    project_dir: &Path,
) -> Result<()> {
    let crate_name = resolve_crate_name(name);

    // Step 1: Read the battery pack spec WITHOUT modifying any manifests.
    // For registry deps: download from crates.io and parse directly.
    // For path deps: read the Cargo.toml from the local path.
    // [impl cli.path.flag]
    // [impl cli.path.no-resolve]
    let (bp_version, bp_spec) = if let Some(local_path) = path {
        let manifest_path = Path::new(local_path).join("Cargo.toml");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        let spec = bphelper_manifest::parse_battery_pack(&manifest_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse battery pack '{}': {}", crate_name, e))?;
        (None, spec)
    } else {
        let (version, spec) = fetch_bp_spec_from_registry(&crate_name)?;
        (Some(version), spec)
    };

    // Step 2: Determine which crates to install — interactive picker, explicit flags, or defaults.
    // No manifest changes have been made yet, so cancellation is free.
    let resolved = resolve_add_crates(
        &bp_spec,
        &crate_name,
        with_features,
        no_default_features,
        all_features,
        specific_crates,
    );
    let (active_features, crates_to_sync) = match resolved {
        ResolvedAdd::Crates {
            active_features,
            crates,
        } => (active_features, crates),
        ResolvedAdd::Interactive if std::io::stdout().is_terminal() => {
            match pick_crates_interactive(&bp_spec)? {
                Some(result) => (result.active_features, result.crates),
                None => {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
        }
        ResolvedAdd::Interactive => {
            // Non-interactive fallback: use defaults
            let crates = bp_spec.resolve_crates(&["default"]);
            (BTreeSet::from(["default".to_string()]), crates)
        }
    };

    if crates_to_sync.is_empty() {
        println!("No crates selected.");
        return Ok(());
    }

    // Step 3: Now write everything — build-dep, workspace deps, crate deps, metadata.
    let user_manifest_path = find_user_manifest(project_dir)?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;
    // [impl manifest.toml.preserve]
    let mut user_doc: toml_edit::DocumentMut = user_manifest_content
        .parse()
        .context("Failed to parse Cargo.toml")?;

    // [impl manifest.register.workspace-default]
    let workspace_manifest = find_workspace_manifest(&user_manifest_path)?;

    // Add battery pack to [build-dependencies]
    let build_deps =
        user_doc["build-dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    if let Some(table) = build_deps.as_table_mut() {
        if let Some(local_path) = path {
            let mut dep = toml_edit::InlineTable::new();
            dep.insert("path", toml_edit::Value::from(local_path));
            table.insert(
                &crate_name,
                toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
            );
        } else if workspace_manifest.is_some() {
            let mut dep = toml_edit::InlineTable::new();
            dep.insert("workspace", toml_edit::Value::from(true));
            table.insert(
                &crate_name,
                toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
            );
        } else {
            table.insert(&crate_name, toml_edit::value(bp_version.as_ref().unwrap()));
        }
    }

    // [impl manifest.deps.workspace]
    // Add crate dependencies + workspace deps (including the battery pack itself)
    if let Some(ref ws_path) = workspace_manifest {
        let ws_content =
            std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
        // [impl manifest.toml.preserve]
        let mut ws_doc: toml_edit::DocumentMut = ws_content
            .parse()
            .context("Failed to parse workspace Cargo.toml")?;

        let ws_deps = ws_doc["workspace"]["dependencies"]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(ws_table) = ws_deps.as_table_mut() {
            // Add the battery pack itself to workspace deps
            if let Some(local_path) = path {
                let mut dep = toml_edit::InlineTable::new();
                dep.insert("path", toml_edit::Value::from(local_path));
                ws_table.insert(
                    &crate_name,
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
                );
            } else {
                ws_table.insert(&crate_name, toml_edit::value(bp_version.as_ref().unwrap()));
            }
            // Add the resolved crate dependencies
            for (dep_name, dep_spec) in &crates_to_sync {
                add_dep_to_table(ws_table, dep_name, dep_spec);
            }
        }
        // [impl manifest.toml.preserve]
        std::fs::write(ws_path, ws_doc.to_string())
            .context("Failed to write workspace Cargo.toml")?;

        // [impl cli.add.dep-kind]
        write_workspace_refs_by_kind(&mut user_doc, &crates_to_sync);
    } else {
        // [impl manifest.deps.no-workspace]
        // [impl cli.add.dep-kind]
        write_deps_by_kind(&mut user_doc, &crates_to_sync);
    }

    // [impl manifest.register.location]
    // [impl manifest.register.format]
    // [impl manifest.features.storage]
    // [impl cli.add.target]
    // Record active features — location depends on --target flag
    let use_workspace_metadata = match target {
        Some(AddTarget::Workspace) => true,
        Some(AddTarget::Package) => false,
        Some(AddTarget::Default) | None => workspace_manifest.is_some(),
    };

    if use_workspace_metadata {
        if let Some(ref ws_path) = workspace_manifest {
            let ws_content =
                std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
            let mut ws_doc: toml_edit::DocumentMut = ws_content
                .parse()
                .context("Failed to parse workspace Cargo.toml")?;
            write_bp_features_to_doc(
                &mut ws_doc,
                &["workspace", "metadata"],
                &crate_name,
                &active_features,
            );
            std::fs::write(ws_path, ws_doc.to_string())
                .context("Failed to write workspace Cargo.toml")?;
        } else {
            bail!("--target=workspace requires a workspace, but none was found");
        }
    } else {
        write_bp_features_to_doc(
            &mut user_doc,
            &["package", "metadata"],
            &crate_name,
            &active_features,
        );
    }

    // Write the final Cargo.toml
    // [impl manifest.toml.preserve]
    std::fs::write(&user_manifest_path, user_doc.to_string())
        .context("Failed to write Cargo.toml")?;

    // Create/modify build.rs
    let build_rs_path = user_manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("build.rs");
    update_build_rs(&build_rs_path, &crate_name)?;

    println!(
        "Added {} with {} crate(s)",
        crate_name,
        crates_to_sync.len()
    );
    for dep_name in crates_to_sync.keys() {
        println!("  + {}", dep_name);
    }

    Ok(())
}

// [impl cli.sync.update-versions]
// [impl cli.sync.add-features]
// [impl cli.sync.add-crates]
// [impl cli.sync.non-destructive]
fn sync_battery_packs(project_dir: &Path) -> Result<()> {
    let user_manifest_path = find_user_manifest(project_dir)?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    let bp_names = find_installed_bp_names(&user_manifest_content)?;

    if bp_names.is_empty() {
        println!("No battery packs installed.");
        return Ok(());
    }

    // [impl manifest.toml.preserve]
    let mut user_doc: toml_edit::DocumentMut = user_manifest_content
        .parse()
        .context("Failed to parse Cargo.toml")?;

    let workspace_manifest = find_workspace_manifest(&user_manifest_path)?;
    let metadata_location = resolve_metadata_location(&user_manifest_path)?;
    let mut total_changes = 0;

    for bp_name in &bp_names {
        // Get the battery pack spec
        let bp_spec = fetch_battery_pack_spec(bp_name)?;

        // Read active features from the correct metadata location
        let active_features =
            read_active_features_from(&metadata_location, &user_manifest_content, bp_name);

        // [impl format.hidden.effect]
        let expected = if active_features.iter().any(|s| s == "all") {
            bp_spec.resolve_all_visible()
        } else {
            let str_features: Vec<&str> = active_features.iter().map(|s| s.as_str()).collect();
            bp_spec.resolve_crates(&str_features)
        };

        // [impl manifest.deps.workspace]
        // Sync each crate
        if let Some(ref ws_path) = workspace_manifest {
            let ws_content =
                std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
            // [impl manifest.toml.preserve]
            let mut ws_doc: toml_edit::DocumentMut = ws_content
                .parse()
                .context("Failed to parse workspace Cargo.toml")?;

            let ws_deps = ws_doc["workspace"]["dependencies"]
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(ws_table) = ws_deps.as_table_mut() {
                for (dep_name, dep_spec) in &expected {
                    if sync_dep_in_table(ws_table, dep_name, dep_spec) {
                        total_changes += 1;
                        println!("  ~ {} (updated in workspace)", dep_name);
                    }
                }
            }
            // [impl manifest.toml.preserve]
            std::fs::write(ws_path, ws_doc.to_string())
                .context("Failed to write workspace Cargo.toml")?;

            // Ensure crate-level references exist in the correct sections
            // [impl cli.add.dep-kind]
            for (dep_name, dep_spec) in &expected {
                let section = dep_kind_section(dep_spec.dep_kind);
                let table =
                    user_doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(table) = table.as_table_mut() {
                    if !table.contains_key(dep_name) {
                        let mut dep = toml_edit::InlineTable::new();
                        dep.insert("workspace", toml_edit::Value::from(true));
                        table.insert(
                            dep_name,
                            toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
                        );
                        total_changes += 1;
                        println!("  + {} (added workspace reference)", dep_name);
                    }
                }
            }
        } else {
            // [impl manifest.deps.no-workspace]
            // [impl cli.add.dep-kind]
            for (dep_name, dep_spec) in &expected {
                let section = dep_kind_section(dep_spec.dep_kind);
                let table =
                    user_doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(table) = table.as_table_mut() {
                    if !table.contains_key(dep_name) {
                        add_dep_to_table(table, dep_name, dep_spec);
                        total_changes += 1;
                        println!("  + {}", dep_name);
                    } else if sync_dep_in_table(table, dep_name, dep_spec) {
                        total_changes += 1;
                        println!("  ~ {}", dep_name);
                    }
                }
            }
        }
    }

    // [impl manifest.toml.preserve]
    std::fs::write(&user_manifest_path, user_doc.to_string())
        .context("Failed to write Cargo.toml")?;

    if total_changes == 0 {
        println!("All dependencies are up to date.");
    } else {
        println!("Synced {} change(s).", total_changes);
    }

    Ok(())
}

fn enable_feature(
    feature_name: &str,
    battery_pack: Option<&str>,
    project_dir: &Path,
) -> Result<()> {
    let user_manifest_path = find_user_manifest(project_dir)?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    // Find which battery pack has this feature
    let bp_name = if let Some(name) = battery_pack {
        resolve_crate_name(name)
    } else {
        // Search all installed battery packs
        let bp_names = find_installed_bp_names(&user_manifest_content)?;

        let mut found = None;
        for name in &bp_names {
            let spec = fetch_battery_pack_spec(name)?;
            if spec.features.contains_key(feature_name) {
                found = Some(name.clone());
                break;
            }
        }
        found.ok_or_else(|| {
            anyhow::anyhow!(
                "No installed battery pack defines feature '{}'",
                feature_name
            )
        })?
    };

    let bp_spec = fetch_battery_pack_spec(&bp_name)?;

    if !bp_spec.features.contains_key(feature_name) {
        let available: Vec<_> = bp_spec.features.keys().collect();
        bail!(
            "Battery pack '{}' has no feature '{}'. Available: {:?}",
            bp_name,
            feature_name,
            available
        );
    }

    // Add feature to active features
    let metadata_location = resolve_metadata_location(&user_manifest_path)?;
    let mut active_features =
        read_active_features_from(&metadata_location, &user_manifest_content, &bp_name);
    if active_features.contains(&feature_name.to_string()) {
        println!(
            "Feature '{}' is already active for {}.",
            feature_name, bp_name
        );
        return Ok(());
    }
    active_features.insert(feature_name.to_string());

    // Resolve what this changes
    let str_features: Vec<&str> = active_features.iter().map(|s| s.as_str()).collect();
    let crates_to_sync = bp_spec.resolve_crates(&str_features);

    // Update user manifest
    let mut user_doc: toml_edit::DocumentMut = user_manifest_content
        .parse()
        .context("Failed to parse Cargo.toml")?;

    let workspace_manifest = find_workspace_manifest(&user_manifest_path)?;

    // Sync the new crates
    if let Some(ref ws_path) = workspace_manifest {
        let ws_content =
            std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
        let mut ws_doc: toml_edit::DocumentMut = ws_content
            .parse()
            .context("Failed to parse workspace Cargo.toml")?;

        let ws_deps = ws_doc["workspace"]["dependencies"]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(ws_table) = ws_deps.as_table_mut() {
            for (dep_name, dep_spec) in &crates_to_sync {
                add_dep_to_table(ws_table, dep_name, dep_spec);
            }
        }
        std::fs::write(ws_path, ws_doc.to_string())
            .context("Failed to write workspace Cargo.toml")?;

        // [impl cli.add.dep-kind]
        for (dep_name, dep_spec) in &crates_to_sync {
            let section = dep_kind_section(dep_spec.dep_kind);
            let table =
                user_doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(table) = table.as_table_mut() {
                if !table.contains_key(dep_name) {
                    let mut dep = toml_edit::InlineTable::new();
                    dep.insert("workspace", toml_edit::Value::from(true));
                    table.insert(
                        dep_name,
                        toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
                    );
                }
            }
        }
    } else {
        // [impl cli.add.dep-kind]
        for (dep_name, dep_spec) in &crates_to_sync {
            let section = dep_kind_section(dep_spec.dep_kind);
            let table =
                user_doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(table) = table.as_table_mut() {
                if !table.contains_key(dep_name) {
                    add_dep_to_table(table, dep_name, dep_spec);
                }
            }
        }
    }

    // Update active features in the correct metadata location
    match &metadata_location {
        MetadataLocation::Workspace { ws_manifest_path } => {
            let ws_content = std::fs::read_to_string(ws_manifest_path)
                .context("Failed to read workspace Cargo.toml")?;
            let mut ws_doc: toml_edit::DocumentMut = ws_content
                .parse()
                .context("Failed to parse workspace Cargo.toml")?;
            write_bp_features_to_doc(
                &mut ws_doc,
                &["workspace", "metadata"],
                &bp_name,
                &active_features,
            );
            std::fs::write(ws_manifest_path, ws_doc.to_string())
                .context("Failed to write workspace Cargo.toml")?;
        }
        MetadataLocation::Package => {
            write_bp_features_to_doc(
                &mut user_doc,
                &["package", "metadata"],
                &bp_name,
                &active_features,
            );
        }
    }

    std::fs::write(&user_manifest_path, user_doc.to_string())
        .context("Failed to write Cargo.toml")?;

    println!("Enabled feature '{}' from {}", feature_name, bp_name);
    Ok(())
}

// ============================================================================
// Interactive crate picker
// ============================================================================

/// Represents the result of an interactive crate selection.
struct PickerResult {
    /// The resolved crates to install (name -> dep spec with merged features).
    crates: BTreeMap<String, bphelper_manifest::CrateSpec>,
    /// Which feature names are fully selected (for metadata recording).
    active_features: BTreeSet<String>,
}

/// Show an interactive multi-select picker for choosing which crates to install.
///
/// Returns `None` if the user cancels. Returns `Some(PickerResult)` with the
/// selected crates and which sets are fully active.
fn pick_crates_interactive(
    bp_spec: &bphelper_manifest::BatteryPackSpec,
) -> Result<Option<PickerResult>> {
    use console::style;
    use dialoguer::MultiSelect;

    let grouped = bp_spec.all_crates_with_grouping();
    if grouped.is_empty() {
        bail!("Battery pack has no crates to add");
    }

    // Build display items and track which group each belongs to
    let mut labels = Vec::new();
    let mut defaults = Vec::new();

    for (group, crate_name, dep, is_default) in &grouped {
        let version_info = if dep.features.is_empty() {
            format!("({})", dep.version)
        } else {
            format!(
                "({}, features: {})",
                dep.version,
                dep.features
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let group_label = if group == "default" {
            String::new()
        } else {
            format!(" [{}]", group)
        };

        labels.push(format!(
            "{} {}{}",
            crate_name,
            style(&version_info).dim(),
            style(&group_label).cyan()
        ));
        defaults.push(*is_default);
    }

    // Show the picker
    println!();
    println!(
        "  {} v{}",
        style(&bp_spec.name).green().bold(),
        style(&bp_spec.version).dim()
    );
    println!();

    let selections = MultiSelect::new()
        .with_prompt("Select crates to add")
        .items(&labels)
        .defaults(&defaults)
        .interact_opt()
        .context("Failed to show crate picker")?;

    let Some(selected_indices) = selections else {
        return Ok(None); // User cancelled
    };

    // Build the result: resolve selected crates with proper feature merging
    let mut crates = BTreeMap::new();

    for idx in &selected_indices {
        let (_group, crate_name, dep, _) = &grouped[*idx];
        // Start with base dep spec
        let merged = (*dep).clone();

        crates.insert(crate_name.clone(), merged);
    }

    // Determine which features are "fully selected" for metadata
    let mut active_features = BTreeSet::from(["default".to_string()]);
    for (feature_name, feature_crates) in &bp_spec.features {
        if feature_name == "default" {
            continue;
        }
        let all_selected = feature_crates.iter().all(|c| crates.contains_key(c));
        if all_selected {
            active_features.insert(feature_name.clone());
        }
    }

    Ok(Some(PickerResult {
        crates,
        active_features,
    }))
}

// ============================================================================
// Cargo.toml manipulation helpers
// ============================================================================

/// Find the user's Cargo.toml in the given directory.
fn find_user_manifest(project_dir: &Path) -> Result<std::path::PathBuf> {
    let path = project_dir.join("Cargo.toml");
    if path.exists() {
        Ok(path)
    } else {
        bail!("No Cargo.toml found in {}", project_dir.display());
    }
}

/// Extract battery pack crate names from a parsed Cargo.toml.
///
/// Filters `[build-dependencies]` for entries ending in `-battery-pack` or equal to `"battery-pack"`.
// [impl manifest.register.location]
pub fn find_installed_bp_names(manifest_content: &str) -> Result<Vec<String>> {
    let raw: toml::Value =
        toml::from_str(manifest_content).context("Failed to parse Cargo.toml")?;

    let build_deps = raw
        .get("build-dependencies")
        .and_then(|bd| bd.as_table())
        .cloned()
        .unwrap_or_default();

    Ok(build_deps
        .keys()
        .filter(|k| k.ends_with("-battery-pack") || *k == "battery-pack")
        .cloned()
        .collect())
}

/// Find the workspace root Cargo.toml, if any.
/// Returns None if the crate is not in a workspace.
// [impl manifest.register.workspace-default]
// [impl manifest.register.both-levels]
fn find_workspace_manifest(crate_manifest: &Path) -> Result<Option<std::path::PathBuf>> {
    let parent = crate_manifest.parent().unwrap_or(Path::new("."));
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let crate_dir = parent
        .canonicalize()
        .context("Failed to resolve crate directory")?;

    // Walk up from the crate directory looking for a workspace root
    let mut dir = crate_dir.clone();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() && candidate != crate_dir.join("Cargo.toml") {
            let content = std::fs::read_to_string(&candidate)?;
            if content.contains("[workspace]") {
                return Ok(Some(candidate));
            }
        }
        if !dir.pop() {
            break;
        }
    }

    // Also check if the crate's own Cargo.toml has a [workspace] section
    // (single-crate workspace) — in that case we don't use workspace deps
    Ok(None)
}

/// Return the TOML section name for a dependency kind.
fn dep_kind_section(kind: bphelper_manifest::DepKind) -> &'static str {
    match kind {
        bphelper_manifest::DepKind::Normal => "dependencies",
        bphelper_manifest::DepKind::Dev => "dev-dependencies",
        bphelper_manifest::DepKind::Build => "build-dependencies",
    }
}

/// Write crates to the correct dependency sections based on their `dep_kind`.
///
/// Groups crates by kind and inserts each into the appropriate
/// `[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]` table.
// [impl cli.add.dep-kind]
fn write_deps_by_kind(
    doc: &mut toml_edit::DocumentMut,
    crates: &BTreeMap<String, bphelper_manifest::CrateSpec>,
) {
    for (dep_name, dep_spec) in crates {
        let section = dep_kind_section(dep_spec.dep_kind);
        let table = doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = table.as_table_mut() {
            add_dep_to_table(table, dep_name, dep_spec);
        }
    }
}

/// Write workspace references (`{ workspace = true }`) to the correct
/// dependency sections based on each crate's `dep_kind`.
// [impl cli.add.dep-kind]
fn write_workspace_refs_by_kind(
    doc: &mut toml_edit::DocumentMut,
    crates: &BTreeMap<String, bphelper_manifest::CrateSpec>,
) {
    for (dep_name, dep_spec) in crates {
        let section = dep_kind_section(dep_spec.dep_kind);
        let table = doc[section].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = table.as_table_mut() {
            let mut dep = toml_edit::InlineTable::new();
            dep.insert("workspace", toml_edit::Value::from(true));
            table.insert(
                dep_name,
                toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
            );
        }
    }
}

/// Add a dependency to a toml_edit table (non-workspace mode).
// [impl manifest.deps.add]
// [impl manifest.deps.version-features]
// [impl manifest.toml.style]
// [impl cli.add.idempotent]
pub fn add_dep_to_table(
    table: &mut toml_edit::Table,
    name: &str,
    spec: &bphelper_manifest::CrateSpec,
) {
    if spec.features.is_empty() {
        table.insert(name, toml_edit::value(&spec.version));
    } else {
        let mut dep = toml_edit::InlineTable::new();
        dep.insert("version", toml_edit::Value::from(spec.version.as_str()));
        let mut features = toml_edit::Array::new();
        for feat in &spec.features {
            features.push(feat.as_str());
        }
        dep.insert("features", toml_edit::Value::Array(features));
        table.insert(
            name,
            toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
        );
    }
}

/// Return true when `recommended` is strictly newer than `current` (semver).
///
/// Falls back to string equality when either side is not a valid semver
/// version, so non-standard version strings still get updated when they
/// differ.
fn should_upgrade_version(current: &str, recommended: &str) -> bool {
    match (
        semver::Version::parse(current)
            .or_else(|_| semver::Version::parse(&format!("{}.0", current)))
            .or_else(|_| semver::Version::parse(&format!("{}.0.0", current))),
        semver::Version::parse(recommended)
            .or_else(|_| semver::Version::parse(&format!("{}.0", recommended)))
            .or_else(|_| semver::Version::parse(&format!("{}.0.0", recommended))),
    ) {
        // [impl manifest.sync.version-bump]
        // [impl manifest.sync.no-downgrade]
        (Ok(cur), Ok(rec)) => rec > cur,
        // Non-parseable: fall back to "update if different"
        _ => current != recommended,
    }
}

/// Sync a dependency in-place: update version if behind, add missing features.
/// Returns true if changes were made.
// [impl manifest.deps.existing]
// [impl manifest.toml.style]
pub fn sync_dep_in_table(
    table: &mut toml_edit::Table,
    name: &str,
    spec: &bphelper_manifest::CrateSpec,
) -> bool {
    let Some(existing) = table.get_mut(name) else {
        // Not present — add it
        add_dep_to_table(table, name, spec);
        return true;
    };

    let mut changed = false;

    match existing {
        toml_edit::Item::Value(toml_edit::Value::String(version_str)) => {
            // Simple version string — check if we need to upgrade
            let current = version_str.value().to_string();
            // [impl manifest.sync.version-bump]
            // [impl manifest.sync.no-downgrade]
            if !spec.version.is_empty() && should_upgrade_version(&current, &spec.version) {
                *version_str = toml_edit::Formatted::new(spec.version.clone());
                changed = true;
            }
            // [impl manifest.sync.feature-add]
            if !spec.features.is_empty() {
                // Need to convert from simple string to table format;
                // use the higher of the two versions so we never downgrade.
                let keep_version = if !spec.version.is_empty()
                    && should_upgrade_version(&current, &spec.version)
                {
                    spec.version.clone()
                } else {
                    current.clone()
                };
                let patched = bphelper_manifest::CrateSpec {
                    version: keep_version,
                    features: spec.features.clone(),
                    dep_kind: spec.dep_kind.clone(),
                    optional: spec.optional,
                };
                add_dep_to_table(table, name, &patched);
                changed = true;
            }
        }
        toml_edit::Item::Value(toml_edit::Value::InlineTable(inline)) => {
            // Check version
            // [impl manifest.sync.version-bump]
            // [impl manifest.sync.no-downgrade]
            if let Some(toml_edit::Value::String(v)) = inline.get_mut("version") {
                if !spec.version.is_empty() && should_upgrade_version(v.value(), &spec.version) {
                    *v = toml_edit::Formatted::new(spec.version.clone());
                    changed = true;
                }
            }
            // [impl manifest.sync.feature-add]
            // [impl manifest.sync.no-feature-remove]
            // Check features — add missing ones, never remove existing
            if !spec.features.is_empty() {
                let existing_features: Vec<String> = inline
                    .get("features")
                    .and_then(|f| f.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut needs_update = false;
                // [impl manifest.sync.no-feature-remove]
                let mut all_features = existing_features.clone();
                for feat in &spec.features {
                    if !existing_features.contains(feat) {
                        all_features.push(feat.clone());
                        needs_update = true;
                    }
                }

                if needs_update {
                    let mut arr = toml_edit::Array::new();
                    for f in &all_features {
                        arr.push(f.as_str());
                    }
                    inline.insert("features", toml_edit::Value::Array(arr));
                    changed = true;
                }
            }
        }
        _ => {}
    }

    changed
}

/// Read active features for a battery pack from user's metadata.
// [impl manifest.features.default-implicit]
// [impl manifest.features.short-form]
pub fn read_active_features(manifest_content: &str, bp_name: &str) -> BTreeSet<String> {
    let raw: toml::Value = match toml::from_str(manifest_content) {
        Ok(v) => v,
        Err(_) => return BTreeSet::from(["default".to_string()]),
    };

    raw.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("battery-pack"))
        .and_then(|bp| bp.get(bp_name))
        .and_then(|entry| entry.get("features"))
        .and_then(|sets| sets.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| BTreeSet::from(["default".to_string()]))
}

// ============================================================================
// Metadata location abstraction
// ============================================================================

/// Where battery-pack metadata (registrations, active features) is stored.
///
/// `add_battery_pack` writes to either `package.metadata` or `workspace.metadata`
/// depending on the `--target` flag. All other commands (sync, enable, load) must
/// read from the same location, so they use `resolve_metadata_location` to detect
/// where metadata currently lives.
#[derive(Debug, Clone)]
enum MetadataLocation {
    /// `[package.metadata.battery-pack]` in the user manifest.
    Package,
    /// `[workspace.metadata.battery-pack]` in the workspace manifest.
    Workspace { ws_manifest_path: PathBuf },
}

/// Determine where battery-pack metadata lives for this project.
///
/// If a workspace manifest exists AND already contains
/// `[workspace.metadata.battery-pack]`, returns `Workspace`.
/// Otherwise returns `Package`.
fn resolve_metadata_location(user_manifest_path: &Path) -> Result<MetadataLocation> {
    if let Some(ws_path) = find_workspace_manifest(user_manifest_path)? {
        let ws_content =
            std::fs::read_to_string(&ws_path).context("Failed to read workspace Cargo.toml")?;
        let raw: toml::Value =
            toml::from_str(&ws_content).context("Failed to parse workspace Cargo.toml")?;
        if raw
            .get("workspace")
            .and_then(|w| w.get("metadata"))
            .and_then(|m| m.get("battery-pack"))
            .is_some()
        {
            return Ok(MetadataLocation::Workspace {
                ws_manifest_path: ws_path,
            });
        }
    }
    Ok(MetadataLocation::Package)
}

/// Read active features for a battery pack, respecting metadata location.
///
/// Dispatches to `read_active_features` (package) or `read_active_features_ws`
/// (workspace) based on the resolved location.
fn read_active_features_from(
    location: &MetadataLocation,
    user_manifest_content: &str,
    bp_name: &str,
) -> BTreeSet<String> {
    match location {
        MetadataLocation::Package => read_active_features(user_manifest_content, bp_name),
        MetadataLocation::Workspace { ws_manifest_path } => {
            let ws_content = match std::fs::read_to_string(ws_manifest_path) {
                Ok(c) => c,
                Err(_) => return BTreeSet::from(["default".to_string()]),
            };
            read_active_features_ws(&ws_content, bp_name)
        }
    }
}

/// Read active features from `workspace.metadata.battery-pack[bp_name].features`.
///
/// Mirrors `read_active_features` but looks in the workspace metadata table
/// instead of the package metadata table.
pub fn read_active_features_ws(ws_content: &str, bp_name: &str) -> BTreeSet<String> {
    let raw: toml::Value = match toml::from_str(ws_content) {
        Ok(v) => v,
        Err(_) => return BTreeSet::from(["default".to_string()]),
    };

    raw.get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("battery-pack"))
        .and_then(|bp| bp.get(bp_name))
        .and_then(|entry| entry.get("features"))
        .and_then(|sets| sets.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| BTreeSet::from(["default".to_string()]))
}

/// Write a features array into a `toml_edit::DocumentMut` at a given path prefix.
///
/// `path_prefix` is `["package", "metadata"]` for package metadata or
/// `["workspace", "metadata"]` for workspace metadata.
fn write_bp_features_to_doc(
    doc: &mut toml_edit::DocumentMut,
    path_prefix: &[&str],
    bp_name: &str,
    active_features: &BTreeSet<String>,
) {
    let mut features_array = toml_edit::Array::new();
    for feature in active_features {
        features_array.push(feature.as_str());
    }

    // Ensure intermediate tables exist (nested, not top-level)
    // path_prefix is e.g. ["package", "metadata"] or ["workspace", "metadata"]
    doc[path_prefix[0]].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    doc[path_prefix[0]][path_prefix[1]].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    doc[path_prefix[0]][path_prefix[1]]["battery-pack"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));

    let bp_meta = &mut doc[path_prefix[0]][path_prefix[1]]["battery-pack"][bp_name];
    *bp_meta = toml_edit::Item::Table(toml_edit::Table::new());
    bp_meta["features"] = toml_edit::value(features_array);
}

/// Resolve the manifest path for a battery pack using `cargo metadata`.
///
/// Works for any dependency source: path deps, registry deps, git deps.
/// The battery pack must already be in [build-dependencies].
fn resolve_battery_pack_manifest(bp_name: &str) -> Result<std::path::PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .context("Failed to run `cargo metadata`")?;

    let package = metadata
        .packages
        .iter()
        .find(|p| p.name == bp_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Battery pack '{}' not found in dependency graph. Is it in [build-dependencies]?",
                bp_name
            )
        })?;

    Ok(package.manifest_path.clone().into())
}

/// Fetch the battery pack spec using `cargo metadata` to locate the manifest.
fn fetch_battery_pack_spec(bp_name: &str) -> Result<bphelper_manifest::BatteryPackSpec> {
    let manifest_path = resolve_battery_pack_manifest(bp_name)?;
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    bphelper_manifest::parse_battery_pack(&manifest_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse battery pack '{}': {}", bp_name, e))
}

/// Download a battery pack from crates.io and parse its spec.
///
/// Unlike `fetch_battery_pack_spec` (which uses cargo metadata and requires the
/// crate to already be a build-dependency), this downloads from the registry
/// directly. Returns `(version, spec)`.
pub(crate) fn fetch_bp_spec_from_registry(
    crate_name: &str,
) -> Result<(String, bphelper_manifest::BatteryPackSpec)> {
    let crate_info = lookup_crate(crate_name)?;
    let temp_dir = download_and_extract_crate(crate_name, &crate_info.version)?;
    let crate_dir = temp_dir
        .path()
        .join(format!("{}-{}", crate_name, crate_info.version));

    let manifest_path = crate_dir.join("Cargo.toml");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let spec = bphelper_manifest::parse_battery_pack(&manifest_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse battery pack '{}': {}", crate_name, e))?;

    Ok((crate_info.version, spec))
}

// ============================================================================
// build.rs manipulation
// ============================================================================

/// Update or create build.rs to include a validate() call.
fn update_build_rs(build_rs_path: &Path, crate_name: &str) -> Result<()> {
    let crate_ident = crate_name.replace('-', "_");
    let validate_call = format!("{}::validate();", crate_ident);

    if build_rs_path.exists() {
        let content = std::fs::read_to_string(build_rs_path).context("Failed to read build.rs")?;

        // Check if validate call is already present
        if content.contains(&validate_call) {
            return Ok(());
        }

        // Verify the file parses as valid Rust with syn
        let file: syn::File = syn::parse_str(&content).context("Failed to parse build.rs")?;

        // Check that a main function exists
        let has_main = file
            .items
            .iter()
            .any(|item| matches!(item, syn::Item::Fn(func) if func.sig.ident == "main"));

        if has_main {
            // Find the closing brace of main using string manipulation
            let lines: Vec<&str> = content.lines().collect();
            let mut insert_line = None;
            let mut brace_depth: i32 = 0;
            let mut in_main = false;

            for (i, line) in lines.iter().enumerate() {
                if line.contains("fn main") {
                    in_main = true;
                    brace_depth = 0;
                }
                if in_main {
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                insert_line = Some(i);
                                in_main = false;
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(line_idx) = insert_line {
                let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
                new_lines.insert(line_idx, format!("    {}", validate_call));
                std::fs::write(build_rs_path, new_lines.join("\n") + "\n")
                    .context("Failed to write build.rs")?;
                return Ok(());
            }
        }

        // Fallback: no main function found or couldn't locate closing brace
        bail!(
            "Could not find fn main() in build.rs. Please add `{}` manually.",
            validate_call
        );
    } else {
        // Create new build.rs
        let content = format!("fn main() {{\n    {}\n}}\n", validate_call);
        std::fs::write(build_rs_path, content).context("Failed to create build.rs")?;
    }

    Ok(())
}

fn generate_from_local(
    local_path: &str,
    name: Option<String>,
    template: Option<String>,
) -> Result<()> {
    let local_path = Path::new(local_path);

    // Read local Cargo.toml
    let manifest_path = local_path.join("Cargo.toml");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let crate_name = local_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let templates = parse_template_metadata(&manifest_content, crate_name)?;
    let template_path = resolve_template(&templates, template.as_deref())?;

    generate_from_path(local_path, &template_path, name)
}

fn generate_from_path(crate_path: &Path, template_path: &str, name: Option<String>) -> Result<()> {
    // In non-interactive mode, provide defaults for placeholders
    let define = if !std::io::stdout().is_terminal() {
        vec!["description=A battery pack for ...".to_string()]
    } else {
        vec![]
    };

    let args = GenerateArgs {
        template_path: TemplatePath {
            path: Some(crate_path.to_string_lossy().into_owned()),
            auto_path: Some(template_path.to_string()),
            ..Default::default()
        },
        name,
        vcs: Some(Vcs::Git),
        define,
        ..Default::default()
    };

    cargo_generate::generate(args)?;

    Ok(())
}

/// Info about a crate from crates.io
struct CrateMetadata {
    version: String,
}

/// Look up a crate on crates.io and return its metadata
fn lookup_crate(crate_name: &str) -> Result<CrateMetadata> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()?;

    let url = format!("{}/{}", CRATES_IO_API, crate_name);
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to query crates.io for '{}'", crate_name))?;

    if !response.status().is_success() {
        bail!(
            "Crate '{}' not found on crates.io (status: {})",
            crate_name,
            response.status()
        );
    }

    let parsed: CratesIoResponse = response
        .json()
        .with_context(|| format!("Failed to parse crates.io response for '{}'", crate_name))?;

    // Find the latest non-yanked version
    let version = parsed
        .versions
        .iter()
        .find(|v| !v.yanked)
        .map(|v| v.num.clone())
        .ok_or_else(|| anyhow::anyhow!("No non-yanked versions found for '{}'", crate_name))?;

    Ok(CrateMetadata { version })
}

/// Download a crate tarball and extract it to a temp directory
fn download_and_extract_crate(crate_name: &str, version: &str) -> Result<tempfile::TempDir> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()?;

    // Download from CDN: https://static.crates.io/crates/{name}/{name}-{version}.crate
    let url = format!(
        "{}/{}/{}-{}.crate",
        CRATES_IO_CDN, crate_name, crate_name, version
    );

    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to download crate from {}", url))?;

    if !response.status().is_success() {
        bail!(
            "Failed to download '{}' version {} (status: {})",
            crate_name,
            version,
            response.status()
        );
    }

    let bytes = response
        .bytes()
        .with_context(|| "Failed to read crate tarball")?;

    // Create temp directory and extract
    let temp_dir = tempfile::tempdir().with_context(|| "Failed to create temp directory")?;

    let decoder = GzDecoder::new(&bytes[..]);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(temp_dir.path())
        .with_context(|| "Failed to extract crate tarball")?;

    Ok(temp_dir)
}

fn parse_template_metadata(
    manifest_content: &str,
    crate_name: &str,
) -> Result<BTreeMap<String, TemplateConfig>> {
    let manifest: CargoManifest =
        toml::from_str(manifest_content).with_context(|| "Failed to parse Cargo.toml")?;

    let templates = manifest
        .package
        .and_then(|p| p.metadata)
        .and_then(|m| m.battery)
        .map(|b| b.templates)
        .unwrap_or_default();

    if templates.is_empty() {
        bail!(
            "Battery pack '{}' has no templates defined in [package.metadata.battery.templates]",
            crate_name
        );
    }

    Ok(templates)
}

// [impl format.templates.selection]
// [impl cli.new.template-select]
pub fn resolve_template(
    templates: &BTreeMap<String, TemplateConfig>,
    requested: Option<&str>,
) -> Result<String> {
    match requested {
        Some(name) => {
            let config = templates.get(name).ok_or_else(|| {
                let available: Vec<_> = templates.keys().map(|s| s.as_str()).collect();
                anyhow::anyhow!(
                    "Template '{}' not found. Available templates: {}",
                    name,
                    available.join(", ")
                )
            })?;
            Ok(config.path.clone())
        }
        None => {
            if templates.len() == 1 {
                // Only one template, use it
                let (_, config) = templates.iter().next().unwrap();
                Ok(config.path.clone())
            } else if let Some(config) = templates.get("default") {
                // Multiple templates, but there's a 'default'
                Ok(config.path.clone())
            } else {
                // Multiple templates, no default - prompt user to pick
                prompt_for_template(templates)
            }
        }
    }
}

fn prompt_for_template(templates: &BTreeMap<String, TemplateConfig>) -> Result<String> {
    use dialoguer::{Select, theme::ColorfulTheme};

    // Build display items with descriptions
    let items: Vec<String> = templates
        .iter()
        .map(|(name, config)| {
            if let Some(desc) = &config.description {
                format!("{} - {}", name, desc)
            } else {
                name.clone()
            }
        })
        .collect();

    // Check if we're in a TTY for interactive mode
    if !std::io::stdout().is_terminal() {
        // Non-interactive: list templates and bail
        println!("Available templates:");
        for item in &items {
            println!("  {}", item);
        }
        bail!("Multiple templates available. Please specify one with --template <name>");
    }

    // Interactive: show selector
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a template")
        .items(&items)
        .default(0)
        .interact()
        .context("Failed to select template")?;

    // Get the selected template's path
    let (_, config) = templates
        .iter()
        .nth(selection)
        .ok_or_else(|| anyhow::anyhow!("Invalid template selection"))?;
    Ok(config.path.clone())
}

/// Info about an installed battery pack — its spec plus which crates are currently enabled.
pub struct InstalledPack {
    pub name: String,
    pub short_name: String,
    pub version: String,
    pub spec: bphelper_manifest::BatteryPackSpec,
    pub active_features: BTreeSet<String>,
}

/// Load all installed battery packs with their specs and active features.
///
/// Reads `[build-dependencies]` from the user's Cargo.toml, fetches each
/// battery pack's spec via cargo metadata, and reads active features from
/// `package.metadata.battery-pack`.
pub fn load_installed_packs(project_dir: &Path) -> Result<Vec<InstalledPack>> {
    let user_manifest_path = find_user_manifest(project_dir)?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    let bp_names = find_installed_bp_names(&user_manifest_content)?;
    let metadata_location = resolve_metadata_location(&user_manifest_path)?;

    let mut packs = Vec::new();
    for bp_name in bp_names {
        let spec = fetch_battery_pack_spec(&bp_name)?;
        let active_features =
            read_active_features_from(&metadata_location, &user_manifest_content, &bp_name);
        packs.push(InstalledPack {
            short_name: short_name(&bp_name).to_string(),
            version: spec.version.clone(),
            spec,
            name: bp_name,
            active_features,
        });
    }

    Ok(packs)
}

/// Fetch battery pack list, dispatching based on source.
pub fn fetch_battery_pack_list(
    source: &CrateSource,
    filter: Option<&str>,
) -> Result<Vec<BatteryPackSummary>> {
    match source {
        CrateSource::Registry => fetch_battery_pack_list_from_registry(filter),
        CrateSource::Local(path) => discover_local_battery_packs(path, filter),
    }
}

fn fetch_battery_pack_list_from_registry(filter: Option<&str>) -> Result<Vec<BatteryPackSummary>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()?;

    // Build the search URL with keyword filter
    let url = match filter {
        Some(q) => format!(
            "{CRATES_IO_API}?q={}&keyword=battery-pack&per_page=50",
            urlencoding::encode(q)
        ),
        None => format!("{CRATES_IO_API}?keyword=battery-pack&per_page=50"),
    };

    let response = client
        .get(&url)
        .send()
        .context("Failed to query crates.io")?;

    if !response.status().is_success() {
        bail!(
            "Failed to list battery packs (status: {})",
            response.status()
        );
    }

    let parsed: SearchResponse = response.json().context("Failed to parse response")?;

    // Filter to only crates whose name ends with "-battery-pack"
    let battery_packs = parsed
        .crates
        .into_iter()
        .filter(|c| c.name.ends_with("-battery-pack"))
        .map(|c| BatteryPackSummary {
            short_name: short_name(&c.name).to_string(),
            name: c.name,
            version: c.max_version,
            description: c.description.unwrap_or_default(),
        })
        .collect();

    Ok(battery_packs)
}

// [impl cli.source.discover]
fn discover_local_battery_packs(
    workspace_dir: &Path,
    filter: Option<&str>,
) -> Result<Vec<BatteryPackSummary>> {
    let manifest_path = workspace_dir.join("Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .with_context(|| format!("Failed to read workspace at {}", manifest_path.display()))?;

    let mut battery_packs: Vec<BatteryPackSummary> = metadata
        .packages
        .iter()
        .filter(|pkg| pkg.name.ends_with("-battery-pack"))
        .filter(|pkg| {
            if let Some(q) = filter {
                short_name(&pkg.name).contains(q)
            } else {
                true
            }
        })
        .map(|pkg| BatteryPackSummary {
            short_name: short_name(&pkg.name).to_string(),
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            description: pkg.description.clone().unwrap_or_default(),
        })
        .collect();

    battery_packs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(battery_packs)
}

fn print_battery_pack_list(source: &CrateSource, filter: Option<&str>) -> Result<()> {
    use console::style;

    let battery_packs = fetch_battery_pack_list(source, filter)?;

    if battery_packs.is_empty() {
        match filter {
            Some(q) => println!("No battery packs found matching '{}'", q),
            None => println!("No battery packs found"),
        }
        return Ok(());
    }

    // Find the longest name for alignment
    let max_name_len = battery_packs
        .iter()
        .map(|c| c.short_name.len())
        .max()
        .unwrap_or(0);

    let max_version_len = battery_packs
        .iter()
        .map(|c| c.version.len())
        .max()
        .unwrap_or(0);

    println!();
    for bp in &battery_packs {
        let desc = bp.description.lines().next().unwrap_or("");

        // Pad strings manually, then apply colors (ANSI codes break width formatting)
        let name_padded = format!("{:<width$}", bp.short_name, width = max_name_len);
        let ver_padded = format!("{:<width$}", bp.version, width = max_version_len);

        println!(
            "  {}  {}  {}",
            style(name_padded).green().bold(),
            style(ver_padded).dim(),
            desc,
        );
    }
    println!();

    println!(
        "{}",
        style(format!("Found {} battery pack(s)", battery_packs.len())).dim()
    );

    Ok(())
}

/// Convert "cli-battery-pack" to "cli" for display
fn short_name(crate_name: &str) -> &str {
    crate_name
        .strip_suffix("-battery-pack")
        .unwrap_or(crate_name)
}

/// Convert "cli" to "cli-battery-pack" (adds suffix if not already present)
/// Special case: "battery-pack" stays as "battery-pack" (not "battery-pack-battery-pack")
// [impl cli.name.resolve]
// [impl cli.name.exact]
fn resolve_crate_name(name: &str) -> String {
    if name == "battery-pack" || name.ends_with("-battery-pack") {
        name.to_string()
    } else {
        format!("{}-battery-pack", name)
    }
}

/// Fetch detailed battery pack info from crates.io or a local path
pub fn fetch_battery_pack_detail(name: &str, path: Option<&str>) -> Result<BatteryPackDetail> {
    // If path is provided, use local directory
    if let Some(local_path) = path {
        return fetch_battery_pack_detail_from_path(local_path);
    }

    let crate_name = resolve_crate_name(name);

    // Look up crate info and download
    let crate_info = lookup_crate(&crate_name)?;
    let temp_dir = download_and_extract_crate(&crate_name, &crate_info.version)?;
    let crate_dir = temp_dir
        .path()
        .join(format!("{}-{}", crate_name, crate_info.version));

    // Fetch owners from crates.io
    let owners = fetch_owners(&crate_name)?;

    build_battery_pack_detail(&crate_dir, crate_name, crate_info.version, owners)
}

/// Fetch detailed battery pack info from a local path
fn fetch_battery_pack_detail_from_path(path: &str) -> Result<BatteryPackDetail> {
    let crate_dir = std::path::Path::new(path);

    // Read Cargo.toml to extract name and version
    let manifest_path = crate_dir.join("Cargo.toml");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: CargoManifest =
        toml::from_str(&manifest_content).with_context(|| "Failed to parse Cargo.toml")?;

    let package = manifest.package.unwrap_or_default();
    let crate_name = package
        .name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let version = package
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());

    build_battery_pack_detail(
        crate_dir,
        crate_name,
        version,
        Vec::new(), // No owners for local path
    )
}

/// Helper function to build BatteryPackDetail from already-resolved parameters.
/// Contains shared logic for both crates.io and local path sources.
fn build_battery_pack_detail(
    crate_dir: &Path,
    crate_name: String,
    version: String,
    owners: Vec<Owner>,
) -> Result<BatteryPackDetail> {
    // Read and parse Cargo.toml
    let manifest_path = crate_dir.join("Cargo.toml");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: CargoManifest =
        toml::from_str(&manifest_content).with_context(|| "Failed to parse Cargo.toml")?;

    // Extract info
    let package = manifest.package.unwrap_or_default();
    let description = package.description.clone().unwrap_or_default();
    let repository = package.repository.clone();
    let battery = package.metadata.and_then(|m| m.battery).unwrap_or_default();

    // Split dependencies into battery packs and regular crates
    let (extends_raw, crates_raw): (Vec<_>, Vec<_>) = manifest
        .dependencies
        .keys()
        .filter(|d| *d != "battery-pack")
        .partition(|d| d.ends_with("-battery-pack"));

    let extends: Vec<String> = extends_raw
        .into_iter()
        .map(|d| short_name(d).to_string())
        .collect();
    let crates: Vec<String> = crates_raw.into_iter().cloned().collect();

    // Fetch the GitHub repository tree to resolve paths
    let repo_tree = repository.as_ref().and_then(|r| fetch_github_tree(r));

    // Convert templates with resolved repo paths
    let templates = battery
        .templates
        .into_iter()
        .map(|(name, config)| {
            let repo_path = repo_tree
                .as_ref()
                .and_then(|tree| find_template_path(tree, &config.path));
            TemplateInfo {
                name,
                path: config.path,
                description: config.description,
                repo_path,
            }
        })
        .collect();

    // Scan examples directory
    let examples = scan_examples(crate_dir, repo_tree.as_deref());

    Ok(BatteryPackDetail {
        short_name: short_name(&crate_name).to_string(),
        name: crate_name,
        version,
        description,
        repository,
        owners: owners.into_iter().map(OwnerInfo::from).collect(),
        crates,
        extends,
        templates,
        examples,
    })
}

// [impl cli.show.details]
// [impl cli.show.hidden]
fn print_battery_pack_detail(name: &str, path: Option<&str>) -> Result<()> {
    use console::style;

    let detail = fetch_battery_pack_detail(name, path)?;

    // Header
    println!();
    println!(
        "{} {}",
        style(&detail.name).green().bold(),
        style(&detail.version).dim()
    );
    if !detail.description.is_empty() {
        println!("{}", detail.description);
    }

    // Authors
    if !detail.owners.is_empty() {
        println!();
        println!("{}", style("Authors:").bold());
        for owner in &detail.owners {
            if let Some(name) = &owner.name {
                println!("  {} ({})", name, owner.login);
            } else {
                println!("  {}", owner.login);
            }
        }
    }

    // Crates
    if !detail.crates.is_empty() {
        println!();
        println!("{}", style("Crates:").bold());
        for dep in &detail.crates {
            println!("  {}", dep);
        }
    }

    // Extends
    if !detail.extends.is_empty() {
        println!();
        println!("{}", style("Extends:").bold());
        for dep in &detail.extends {
            println!("  {}", dep);
        }
    }

    // Templates
    if !detail.templates.is_empty() {
        println!();
        println!("{}", style("Templates:").bold());
        let max_name_len = detail
            .templates
            .iter()
            .map(|t| t.name.len())
            .max()
            .unwrap_or(0);
        for tmpl in &detail.templates {
            let name_padded = format!("{:<width$}", tmpl.name, width = max_name_len);
            if let Some(desc) = &tmpl.description {
                println!("  {}  {}", style(name_padded).cyan(), desc);
            } else {
                println!("  {}", style(name_padded).cyan());
            }
        }
    }

    // [impl format.examples.browsable]
    // Examples
    if !detail.examples.is_empty() {
        println!();
        println!("{}", style("Examples:").bold());
        let max_name_len = detail
            .examples
            .iter()
            .map(|e| e.name.len())
            .max()
            .unwrap_or(0);
        for example in &detail.examples {
            let name_padded = format!("{:<width$}", example.name, width = max_name_len);
            if let Some(desc) = &example.description {
                println!("  {}  {}", style(name_padded).magenta(), desc);
            } else {
                println!("  {}", style(name_padded).magenta());
            }
        }
    }

    // Install hints
    println!();
    println!("{}", style("Install:").bold());
    println!("  cargo bp add {}", detail.short_name);
    println!("  cargo bp new {}", detail.short_name);
    println!();

    Ok(())
}

fn fetch_owners(crate_name: &str) -> Result<Vec<Owner>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()?;

    let url = format!("{}/{}/owners", CRATES_IO_API, crate_name);
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to fetch owners for '{}'", crate_name))?;

    if !response.status().is_success() {
        // Not fatal - just return empty
        return Ok(Vec::new());
    }

    let parsed: OwnersResponse = response
        .json()
        .with_context(|| "Failed to parse owners response")?;

    Ok(parsed.users)
}

/// Scan the examples directory and extract example info.
/// If a GitHub tree is provided, resolves the full repository path for each example.
// [impl format.examples.standard]
fn scan_examples(crate_dir: &std::path::Path, repo_tree: Option<&[String]>) -> Vec<ExampleInfo> {
    let examples_dir = crate_dir.join("examples");
    if !examples_dir.exists() {
        return Vec::new();
    }

    let mut examples = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&examples_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    let description = extract_example_description(&path);
                    let repo_path = repo_tree.and_then(|tree| find_example_path(tree, name));
                    examples.push(ExampleInfo {
                        name: name.to_string(),
                        description,
                        repo_path,
                    });
                }
            }
        }
    }

    // Sort by name
    examples.sort_by(|a, b| a.name.cmp(&b.name));
    examples
}

/// Extract description from the first doc comment in an example file
fn extract_example_description(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;

    // Look for //! doc comments at the start
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            let desc = trimmed.strip_prefix("//!").unwrap_or("").trim();
            if !desc.is_empty() {
                return Some(desc.to_string());
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Stop at first non-comment, non-empty line
            break;
        }
    }
    None
}

/// Fetch the repository tree from GitHub API.
/// Returns a list of all file paths in the repository.
fn fetch_github_tree(repository: &str) -> Option<Vec<String>> {
    // Parse GitHub URL: https://github.com/owner/repo
    let gh_path = repository
        .strip_prefix("https://github.com/")
        .or_else(|| repository.strip_prefix("http://github.com/"))?;
    let gh_path = gh_path.strip_suffix(".git").unwrap_or(gh_path);
    let gh_path = gh_path.trim_end_matches('/');

    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()
        .ok()?;

    // Fetch the tree recursively using the main branch
    let url = format!(
        "https://api.github.com/repos/{}/git/trees/main?recursive=1",
        gh_path
    );

    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let tree_response: GitHubTreeResponse = response.json().ok()?;

    // Extract all paths (both blobs/files and trees/directories)
    Some(tree_response.tree.into_iter().map(|e| e.path).collect())
}

/// Find the full repository path for an example file.
/// Searches the tree for a file matching "examples/{name}.rs".
fn find_example_path(tree: &[String], example_name: &str) -> Option<String> {
    let suffix = format!("examples/{}.rs", example_name);
    tree.iter().find(|path| path.ends_with(&suffix)).cloned()
}

/// Find the full repository path for a template directory.
/// Searches the tree for a path matching "templates/{name}" or "{name}".
fn find_template_path(tree: &[String], template_path: &str) -> Option<String> {
    // The template path from config might be "templates/simple" or just the relative path
    tree.iter()
        .find(|path| path.ends_with(template_path))
        .cloned()
}

// ============================================================================
// Validate command
// ============================================================================

// [impl cli.validate.purpose]
// [impl cli.validate.default-path]
pub fn validate_battery_pack_cmd(path: Option<&str>) -> Result<()> {
    let crate_root = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => std::env::current_dir().context("failed to get current directory")?,
    };

    let cargo_toml = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;

    // Check for virtual/workspace manifest before attempting battery pack parse
    let raw: toml::Value = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}", cargo_toml.display()))?;
    if raw.get("package").is_none() {
        if raw.get("workspace").is_some() {
            // [impl cli.validate.workspace-error]
            bail!(
                "{} is a workspace manifest, not a battery pack crate.\n\
                 Run this from a battery pack crate directory, or use --path to point to one.",
                cargo_toml.display()
            );
        } else {
            // [impl cli.validate.no-package]
            bail!(
                "{} has no [package] section — is this a battery pack crate?",
                cargo_toml.display()
            );
        }
    }

    let spec = bphelper_manifest::parse_battery_pack(&content)
        .with_context(|| format!("failed to parse {}", cargo_toml.display()))?;

    // [impl cli.validate.checks]
    let mut report = spec.validate_spec();
    report.merge(bphelper_manifest::validate_on_disk(&spec, &crate_root));

    // [impl cli.validate.clean]
    if report.is_clean() {
        println!("{} is valid", spec.name);
        return Ok(());
    }

    // [impl cli.validate.severity]
    // [impl cli.validate.rule-id]
    let mut errors = 0;
    let mut warnings = 0;
    for diag in &report.diagnostics {
        match diag.severity {
            bphelper_manifest::Severity::Error => {
                eprintln!("error[{}]: {}", diag.rule, diag.message);
                errors += 1;
            }
            bphelper_manifest::Severity::Warning => {
                eprintln!("warning[{}]: {}", diag.rule, diag.message);
                warnings += 1;
            }
        }
    }

    // [impl cli.validate.errors]
    if errors > 0 {
        bail!(
            "validation failed: {} error(s), {} warning(s)",
            errors,
            warnings
        );
    }

    // [impl cli.validate.warnings-only]
    // Warnings only — still succeeds
    println!("{} is valid ({} warning(s))", spec.name, warnings);
    Ok(())
}
