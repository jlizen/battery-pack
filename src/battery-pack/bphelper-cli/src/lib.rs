//! CLI for battery-pack: create and manage battery packs.

use anyhow::{bail, Context, Result};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use tar::Archive;

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

        /// Use exact crate name without adding "-battery-pack" suffix
        #[arg(long)]
        exact: bool,

        /// Use a local path instead of downloading from crates.io
        #[arg(long)]
        path: Option<String>,
    },

    /// Add a battery pack as a dependency
    Add {
        /// Name of the battery pack (e.g., "cli" resolves to "cli-battery-pack")
        battery_pack: String,

        /// Features to enable
        #[arg(long, short = 'F')]
        features: Vec<String>,
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
                exact,
                path,
            } => new_from_battery_pack(&battery_pack, name, template, exact, path),
            BpCommands::Add {
                battery_pack,
                features,
            } => add_battery_pack(&battery_pack, &features),
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

// ============================================================================
// Battery pack metadata types
// ============================================================================

#[derive(Deserialize, Default)]
struct CargoManifest {
    package: Option<PackageSection>,
}

#[derive(Deserialize, Default)]
struct PackageSection {
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
// Implementation
// ============================================================================

fn new_from_battery_pack(
    battery_pack: &str,
    name: Option<String>,
    template: Option<String>,
    exact: bool,
    path_override: Option<String>,
) -> Result<()> {
    // If using local path, generate directly from there
    if let Some(path) = path_override {
        return generate_from_local(&path, name, template);
    }

    // Resolve the crate name (add -battery-pack suffix unless --exact)
    let crate_name = if exact || battery_pack.ends_with("-battery-pack") {
        battery_pack.to_string()
    } else {
        format!("{}-battery-pack", battery_pack)
    };

    // Look up the crate on crates.io and get the latest version
    let crate_info = lookup_crate(&crate_name)?;

    // Download and extract the crate to a temp directory
    let temp_dir = download_and_extract_crate(&crate_name, &crate_info.version)?;
    let crate_dir = temp_dir.path().join(format!("{}-{}", crate_name, crate_info.version));

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

fn add_battery_pack(name: &str, features: &[String]) -> Result<()> {
    // Resolve the crate name (add -battery-pack suffix if needed)
    let crate_name = if name.ends_with("-battery-pack") {
        name.to_string()
    } else {
        format!("{}-battery-pack", name)
    };

    // Verify the crate exists on crates.io
    lookup_crate(&crate_name)?;

    // Build cargo add command: cargo add cli-battery-pack --rename cli
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("add").arg(&crate_name);

    // Rename to the short name (e.g., cli-battery-pack -> cli)
    if crate_name != name {
        cmd.arg("--rename").arg(name);
    }

    // Add features if specified
    for feature in features {
        cmd.arg("--features").arg(feature);
    }

    let status = cmd.status().context("Failed to run cargo add")?;

    if !status.success() {
        bail!("cargo add failed");
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
    let args = GenerateArgs {
        template_path: TemplatePath {
            path: Some(crate_path.to_string_lossy().into_owned()),
            auto_path: Some(template_path.to_string()),
            ..Default::default()
        },
        name,
        vcs: Some(Vcs::Git),
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
fn download_and_extract_crate(
    crate_name: &str,
    version: &str,
) -> Result<tempfile::TempDir> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-bp (https://github.com/battery-pack-rs/battery-pack)")
        .build()?;

    // Download from CDN: https://static.crates.io/crates/{name}/{name}-{version}.crate
    let url = format!("{}/{}/{}-{}.crate", CRATES_IO_CDN, crate_name, crate_name, version);

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
                // Multiple templates, no default - list them
                println!("Available templates:");
                for (name, config) in templates {
                    if let Some(desc) = &config.description {
                        println!("  {} - {}", name, desc);
                    } else {
                        println!("  {}", name);
                    }
                }
                bail!("Multiple templates available. Please specify one with --template <name>");
            }
        }
    }
}
