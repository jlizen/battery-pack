//! CLI for battery-pack: create and manage battery packs.

use anyhow::{Context, Result, bail};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::Path;
use tar::Archive;

mod tui;

const CRATES_IO_API: &str = "https://crates.io/api/v1/crates";
const CRATES_IO_CDN: &str = "https://static.crates.io/crates";

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
    /// for choosing crates if the pack has sets or many dependencies).
    Add {
        /// Name of the battery pack (e.g., "cli" resolves to "cli-battery-pack").
        /// Omit to open the interactive manager.
        battery_pack: Option<String>,

        /// Named sets to enable (in addition to the default set)
        #[arg(long, short = 'w')]
        with: Vec<String>,

        /// Sync all dev-dependencies regardless of default set
        #[arg(long)]
        all: bool,

        /// Use a local path instead of downloading from crates.io
        #[arg(long)]
        path: Option<String>,
    },

    /// Update dependencies from installed battery packs
    Sync,

    /// Enable a named set from a battery pack
    Enable {
        /// Name of the set to enable
        set_name: String,

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

/// Main entry point for the CLI.
pub fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bp { command } => match command {
            BpCommands::New {
                battery_pack,
                name,
                template,
                path,
            } => new_from_battery_pack(&battery_pack, name, template, path),
            BpCommands::Add {
                battery_pack,
                with,
                all,
                path,
            } => match battery_pack {
                Some(name) => add_battery_pack(&name, &with, all, path.as_deref()),
                None if std::io::stdout().is_terminal() => tui::run_add(),
                None => {
                    bail!(
                        "No battery pack specified. Use `cargo bp add <name>` or run interactively in a terminal."
                    )
                }
            },
            BpCommands::Sync => sync_battery_packs(),
            BpCommands::Enable {
                set_name,
                battery_pack,
            } => enable_set(&set_name, battery_pack.as_deref()),
            BpCommands::List {
                filter,
                non_interactive,
            } => {
                if !non_interactive && std::io::stdout().is_terminal() {
                    tui::run_list(filter)
                } else {
                    print_battery_pack_list(filter.as_deref())
                }
            }
            BpCommands::Show {
                battery_pack,
                path,
                non_interactive,
            } => {
                if !non_interactive && std::io::stdout().is_terminal() {
                    tui::run_show(&battery_pack, path.as_deref())
                } else {
                    print_battery_pack_detail(&battery_pack, path.as_deref())
                }
            }
            BpCommands::Validate { path } => validate_battery_pack_cmd(path.as_deref()),
        },
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
struct TemplateConfig {
    path: String,
    #[serde(default)]
    description: Option<String>,
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

fn add_battery_pack(name: &str, with_sets: &[String], all: bool, path: Option<&str>) -> Result<()> {
    let crate_name = resolve_crate_name(name);

    // Step 1: Read the battery pack spec WITHOUT modifying any manifests.
    // For registry deps: download from crates.io and parse directly.
    // For path deps: read the Cargo.toml from the local path.
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
    let use_picker = !all
        && with_sets.is_empty()
        && std::io::stdout().is_terminal()
        && bp_spec.has_meaningful_choices();

    let (active_sets, crates_to_sync) = if all {
        (vec!["all".to_string()], bp_spec.resolve_all())
    } else if use_picker {
        match pick_crates_interactive(&bp_spec)? {
            Some(result) => (result.active_sets, result.crates),
            None => {
                println!("Cancelled.");
                return Ok(());
            }
        }
    } else {
        let mut sets = vec!["default".to_string()];
        sets.extend(with_sets.iter().cloned());
        let str_sets: Vec<&str> = sets.iter().map(|s| s.as_str()).collect();
        let crates = bp_spec.resolve_crates(&str_sets);
        (sets, crates)
    };

    if crates_to_sync.is_empty() {
        println!("No crates selected.");
        return Ok(());
    }

    // Step 3: Now write everything — build-dep, workspace deps, crate deps, metadata.
    let user_manifest_path = find_user_manifest()?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;
    let mut user_doc: toml_edit::DocumentMut = user_manifest_content
        .parse()
        .context("Failed to parse Cargo.toml")?;

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

    // Add crate dependencies + workspace deps (including the battery pack itself)
    if let Some(ref ws_path) = workspace_manifest {
        let ws_content =
            std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
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
        std::fs::write(ws_path, ws_doc.to_string())
            .context("Failed to write workspace Cargo.toml")?;

        let deps =
            user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = deps.as_table_mut() {
            for dep_name in crates_to_sync.keys() {
                let mut dep = toml_edit::InlineTable::new();
                dep.insert("workspace", toml_edit::Value::from(true));
                table.insert(
                    dep_name,
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(dep)),
                );
            }
        }
    } else {
        let deps =
            user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = deps.as_table_mut() {
            for (dep_name, dep_spec) in &crates_to_sync {
                add_dep_to_table(table, dep_name, dep_spec);
            }
        }
    }

    // Record active sets in [package.metadata.battery-pack.<crate-name>]
    let bp_meta = &mut user_doc["package"]["metadata"]["battery-pack"][&crate_name];
    let mut sets_array = toml_edit::Array::new();
    for set in &active_sets {
        sets_array.push(set.as_str());
    }
    *bp_meta = toml_edit::Item::Table(toml_edit::Table::new());
    bp_meta["sets"] = toml_edit::value(sets_array);

    // Write the final Cargo.toml
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

fn sync_battery_packs() -> Result<()> {
    let user_manifest_path = find_user_manifest()?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    let bp_names = find_installed_bp_names(&user_manifest_content)?;

    if bp_names.is_empty() {
        println!("No battery packs installed.");
        return Ok(());
    }

    let mut user_doc: toml_edit::DocumentMut = user_manifest_content
        .parse()
        .context("Failed to parse Cargo.toml")?;

    let workspace_manifest = find_workspace_manifest(&user_manifest_path)?;
    let mut total_changes = 0;

    for bp_name in &bp_names {
        // Get the battery pack spec
        let bp_spec = fetch_battery_pack_spec(bp_name)?;

        // Read active sets from user metadata
        let active_sets = read_active_sets(&user_manifest_content, bp_name);

        let expected = if active_sets.iter().any(|s| s == "all") {
            bp_spec.resolve_all()
        } else {
            let str_sets: Vec<&str> = active_sets.iter().map(|s| s.as_str()).collect();
            bp_spec.resolve_crates(&str_sets)
        };

        // Sync each crate
        if let Some(ref ws_path) = workspace_manifest {
            let ws_content =
                std::fs::read_to_string(ws_path).context("Failed to read workspace Cargo.toml")?;
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
            std::fs::write(ws_path, ws_doc.to_string())
                .context("Failed to write workspace Cargo.toml")?;

            // Ensure crate-level references exist
            let deps =
                user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(table) = deps.as_table_mut() {
                for dep_name in expected.keys() {
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
            let deps =
                user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(table) = deps.as_table_mut() {
                for (dep_name, dep_spec) in &expected {
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

    std::fs::write(&user_manifest_path, user_doc.to_string())
        .context("Failed to write Cargo.toml")?;

    if total_changes == 0 {
        println!("All dependencies are up to date.");
    } else {
        println!("Synced {} change(s).", total_changes);
    }

    Ok(())
}

fn enable_set(set_name: &str, battery_pack: Option<&str>) -> Result<()> {
    let user_manifest_path = find_user_manifest()?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    // Find which battery pack has this set
    let bp_name = if let Some(name) = battery_pack {
        resolve_crate_name(name)
    } else {
        // Search all installed battery packs
        let bp_names = find_installed_bp_names(&user_manifest_content)?;

        let mut found = None;
        for name in &bp_names {
            let spec = fetch_battery_pack_spec(name)?;
            if spec.features.contains_key(set_name) {
                found = Some(name.clone());
                break;
            }
        }
        found.ok_or_else(|| {
            anyhow::anyhow!("No installed battery pack defines set '{}'", set_name)
        })?
    };

    let bp_spec = fetch_battery_pack_spec(&bp_name)?;

    if !bp_spec.features.contains_key(set_name) {
        let available: Vec<_> = bp_spec.features.keys().collect();
        bail!(
            "Battery pack '{}' has no set '{}'. Available: {:?}",
            bp_name,
            set_name,
            available
        );
    }

    // Add set to active sets
    let mut active_sets = read_active_sets(&user_manifest_content, &bp_name);
    if active_sets.contains(&set_name.to_string()) {
        println!("Set '{}' is already active for {}.", set_name, bp_name);
        return Ok(());
    }
    active_sets.push(set_name.to_string());

    // Resolve what this changes
    let str_sets: Vec<&str> = active_sets.iter().map(|s| s.as_str()).collect();
    let crates_to_sync = bp_spec.resolve_crates(&str_sets);

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

        let deps =
            user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = deps.as_table_mut() {
            for dep_name in crates_to_sync.keys() {
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
        let deps =
            user_doc["dependencies"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if let Some(table) = deps.as_table_mut() {
            for (dep_name, dep_spec) in &crates_to_sync {
                if !table.contains_key(dep_name) {
                    add_dep_to_table(table, dep_name, dep_spec);
                }
            }
        }
    }

    // Update active sets in metadata
    let bp_meta = &mut user_doc["package"]["metadata"]["battery-pack"][&bp_name];
    let mut sets_array = toml_edit::Array::new();
    for set in &active_sets {
        sets_array.push(set.as_str());
    }
    *bp_meta = toml_edit::Item::Table(toml_edit::Table::new());
    bp_meta["sets"] = toml_edit::value(sets_array);

    std::fs::write(&user_manifest_path, user_doc.to_string())
        .context("Failed to write Cargo.toml")?;

    println!("Enabled set '{}' from {}", set_name, bp_name);
    Ok(())
}

// ============================================================================
// Interactive crate picker
// ============================================================================

/// Represents the result of an interactive crate selection.
struct PickerResult {
    /// The resolved crates to install (name -> dep spec with merged features).
    crates: BTreeMap<String, bphelper_manifest::CrateSpec>,
    /// Which set names are fully selected (for metadata recording).
    active_sets: Vec<String>,
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
            format!("({}, features: {})", dep.version, dep.features.join(", "))
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
    let mut active_sets = vec!["default".to_string()];
    for (feature_name, feature_crates) in &bp_spec.features {
        if feature_name == "default" {
            continue;
        }
        let all_selected = feature_crates.iter().all(|c| crates.contains_key(c));
        if all_selected {
            active_sets.push(feature_name.clone());
        }
    }

    Ok(Some(PickerResult {
        crates,
        active_sets,
    }))
}

// ============================================================================
// Cargo.toml manipulation helpers
// ============================================================================

/// Find the user's Cargo.toml in the current directory.
fn find_user_manifest() -> Result<std::path::PathBuf> {
    let path = std::path::PathBuf::from("Cargo.toml");
    if path.exists() {
        Ok(path)
    } else {
        bail!("No Cargo.toml found in the current directory");
    }
}

/// Extract battery pack crate names from a parsed Cargo.toml.
///
/// Filters `[build-dependencies]` for entries ending in `-battery-pack` or equal to `"battery-pack"`.
fn find_installed_bp_names(manifest_content: &str) -> Result<Vec<String>> {
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

/// Add a dependency to a toml_edit table (non-workspace mode).
fn add_dep_to_table(table: &mut toml_edit::Table, name: &str, spec: &bphelper_manifest::CrateSpec) {
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

/// Sync a dependency in-place: update version if behind, add missing features.
/// Returns true if changes were made.
fn sync_dep_in_table(
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
            // Simple version string — check if we need to upgrade or add features
            let current = version_str.value().to_string();
            if !spec.version.is_empty() && current != spec.version {
                *version_str = toml_edit::Formatted::new(spec.version.clone());
                changed = true;
            }
            if !spec.features.is_empty() {
                // Need to convert from simple string to table format
                add_dep_to_table(table, name, spec);
                changed = true;
            }
        }
        toml_edit::Item::Value(toml_edit::Value::InlineTable(inline)) => {
            // Check version
            if let Some(toml_edit::Value::String(v)) = inline.get_mut("version") {
                if !spec.version.is_empty() && v.value() != &spec.version {
                    *v = toml_edit::Formatted::new(spec.version.clone());
                    changed = true;
                }
            }
            // Check features — add missing ones
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

/// Read active sets for a battery pack from user's metadata.
fn read_active_sets(manifest_content: &str, bp_name: &str) -> Vec<String> {
    let raw: toml::Value = match toml::from_str(manifest_content) {
        Ok(v) => v,
        Err(_) => return vec!["default".to_string()],
    };

    raw.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("battery-pack"))
        .and_then(|bp| bp.get(bp_name))
        .and_then(|entry| entry.get("sets"))
        .and_then(|sets| sets.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["default".to_string()])
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

fn resolve_template(
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
    pub active_sets: Vec<String>,
}

/// Load all installed battery packs with their specs and active sets.
///
/// Reads `[build-dependencies]` from the user's Cargo.toml, fetches each
/// battery pack's spec via cargo metadata, and reads active sets from
/// `[package.metadata.battery-pack]`.
pub fn load_installed_packs() -> Result<Vec<InstalledPack>> {
    let user_manifest_path = find_user_manifest()?;
    let user_manifest_content =
        std::fs::read_to_string(&user_manifest_path).context("Failed to read Cargo.toml")?;

    let bp_names = find_installed_bp_names(&user_manifest_content)?;

    let mut packs = Vec::new();
    for bp_name in bp_names {
        let spec = fetch_battery_pack_spec(&bp_name)?;
        let active_sets = read_active_sets(&user_manifest_content, &bp_name);
        packs.push(InstalledPack {
            short_name: short_name(&bp_name).to_string(),
            version: spec.version.clone(),
            spec,
            name: bp_name,
            active_sets,
        });
    }

    Ok(packs)
}

/// Fetch battery pack list from crates.io
pub fn fetch_battery_pack_list(filter: Option<&str>) -> Result<Vec<BatteryPackSummary>> {
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

fn print_battery_pack_list(filter: Option<&str>) -> Result<()> {
    use console::style;

    let battery_packs = fetch_battery_pack_list(filter)?;

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

fn validate_battery_pack_cmd(path: Option<&str>) -> Result<()> {
    let crate_root = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => std::env::current_dir().context("failed to get current directory")?,
    };

    let cargo_toml = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;

    let spec = bphelper_manifest::parse_battery_pack(&content)
        .with_context(|| format!("failed to parse {}", cargo_toml.display()))?;

    let mut report = spec.validate_spec();
    report.merge(bphelper_manifest::validate_on_disk(&spec, &crate_root));

    if report.is_clean() {
        println!("{} is valid", spec.name);
        return Ok(());
    }

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

    if errors > 0 {
        bail!(
            "validation failed: {} error(s), {} warning(s)",
            errors,
            warnings
        );
    }

    // Warnings only — still succeeds
    println!("{} is valid ({} warning(s))", spec.name, warnings);
    Ok(())
}
