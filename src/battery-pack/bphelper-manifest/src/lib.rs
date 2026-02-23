//! Battery pack manifest parsing and drift detection.
//!
//! Parses battery pack Cargo.toml files to extract dev-dependencies,
//! sets, and default configuration. Also parses user manifests to
//! detect drift between expected and actual dependencies.

use serde::Deserialize;
use std::collections::BTreeMap;

// ============================================================================
// Battery pack manifest types
// ============================================================================

/// Parsed battery pack specification.
#[derive(Debug, Clone)]
pub struct BatteryPackSpec {
    pub name: String,
    pub version: String,
    pub dev_dependencies: BTreeMap<String, DepSpec>,
    pub default_crates: Vec<String>,
    pub sets: BTreeMap<String, SetSpec>,
}

/// A dependency specification (version + features).
#[derive(Debug, Clone)]
pub struct DepSpec {
    pub version: String,
    pub features: Vec<String>,
}

/// A named set of crates with optional feature augmentation.
#[derive(Debug, Clone)]
pub struct SetSpec {
    pub crates: BTreeMap<String, SetCrateSpec>,
}

/// A crate entry within a set — may specify additional features.
#[derive(Debug, Clone)]
pub struct SetCrateSpec {
    pub features: Vec<String>,
}

impl BatteryPackSpec {
    /// Resolve a list of active set names into a merged list of (crate, version, features).
    ///
    /// Starts with crates from `default_crates` (looked up in dev_dependencies),
    /// then layers on additional sets. Feature merging is always additive.
    pub fn resolve_crates(&self, active_sets: &[String]) -> BTreeMap<String, DepSpec> {
        let mut result: BTreeMap<String, DepSpec> = BTreeMap::new();

        // Start with default crates
        for crate_name in &self.default_crates {
            if let Some(dep) = self.dev_dependencies.get(crate_name) {
                result.insert(crate_name.clone(), dep.clone());
            }
        }

        // Layer on each active set (skip "default" — already handled)
        for set_name in active_sets {
            if set_name == "default" {
                continue;
            }
            if let Some(set) = self.sets.get(set_name) {
                for (crate_name, set_crate) in &set.crates {
                    if let Some(existing) = result.get_mut(crate_name) {
                        // Crate already present — merge features additively
                        for feat in &set_crate.features {
                            if !existing.features.contains(feat) {
                                existing.features.push(feat.clone());
                            }
                        }
                    } else if let Some(dep) = self.dev_dependencies.get(crate_name) {
                        // New crate from this set — start with base features, add set features
                        let mut features = dep.features.clone();
                        for feat in &set_crate.features {
                            if !features.contains(feat) {
                                features.push(feat.clone());
                            }
                        }
                        result.insert(
                            crate_name.clone(),
                            DepSpec {
                                version: dep.version.clone(),
                                features,
                            },
                        );
                    }
                }
            }
        }

        result
    }

    /// Resolve all dev-dependencies (for --all flag).
    pub fn resolve_all(&self) -> BTreeMap<String, DepSpec> {
        let mut result: BTreeMap<String, DepSpec> = BTreeMap::new();

        for (name, dep) in &self.dev_dependencies {
            result.insert(name.clone(), dep.clone());
        }

        // Apply all set feature augmentations
        for set in self.sets.values() {
            for (crate_name, set_crate) in &set.crates {
                if let Some(existing) = result.get_mut(crate_name) {
                    for feat in &set_crate.features {
                        if !existing.features.contains(feat) {
                            existing.features.push(feat.clone());
                        }
                    }
                }
            }
        }

        result
    }
}

// ============================================================================
// User manifest types
// ============================================================================

/// Parsed user manifest (the parts we care about for validation).
#[derive(Debug)]
pub struct UserManifest {
    pub dependencies: BTreeMap<String, DepSpec>,
    /// Active sets per battery pack: battery-pack-name -> list of set names
    pub battery_pack_sets: BTreeMap<String, Vec<String>>,
}

// ============================================================================
// Raw deserialization types (internal)
// ============================================================================

#[derive(Deserialize)]
struct RawManifest {
    package: Option<RawPackage>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, toml::Value>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    metadata: Option<RawMetadata>,
}

#[derive(Deserialize)]
struct RawMetadata {
    #[serde(default, rename = "battery-pack")]
    battery_pack: Option<RawBatteryPackMetadata>,
}

#[derive(Deserialize)]
struct RawBatteryPackMetadata {
    default: Option<Vec<String>>,
    #[serde(default)]
    sets: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

// ============================================================================
// Parsing functions
// ============================================================================

/// Parse a battery pack's Cargo.toml into a BatteryPackSpec.
pub fn parse_battery_pack(manifest_str: &str) -> Result<BatteryPackSpec, String> {
    let raw: RawManifest =
        toml::from_str(manifest_str).map_err(|e| format!("TOML parse error: {e}"))?;

    let package = raw.package.ok_or("missing [package] section")?;
    let name = package.name.ok_or("missing package.name")?;
    let version = package.version.ok_or("missing package.version")?;

    // Parse dev-dependencies
    let dev_dependencies = parse_dep_map(&raw.dev_dependencies);

    // Parse metadata
    let bp_meta = package.metadata.and_then(|m| m.battery_pack);

    // Default set: if specified, use it; otherwise all dev-deps
    let default_crates = match bp_meta.as_ref().and_then(|m| m.default.as_ref()) {
        Some(explicit) => explicit.clone(),
        None => dev_dependencies.keys().cloned().collect(),
    };

    // Parse sets
    let sets = match bp_meta.as_ref() {
        Some(meta) => parse_sets(&meta.sets),
        None => BTreeMap::new(),
    };

    Ok(BatteryPackSpec {
        name,
        version,
        dev_dependencies,
        default_crates,
        sets,
    })
}

/// Parse a user's Cargo.toml to extract dependencies and battery pack metadata.
pub fn parse_user_manifest(manifest_str: &str) -> Result<UserManifest, String> {
    let raw: RawManifest =
        toml::from_str(manifest_str).map_err(|e| format!("TOML parse error: {e}"))?;

    let dependencies = parse_dep_map(&raw.dependencies);

    // Parse battery pack sets from [package.metadata.battery-pack.<bp-name>]
    let battery_pack_sets = parse_user_bp_sets(manifest_str);

    Ok(UserManifest {
        dependencies,
        battery_pack_sets,
    })
}

fn parse_user_bp_sets(manifest_str: &str) -> BTreeMap<String, Vec<String>> {
    // Parse just the metadata section to extract per-battery-pack sets
    let raw: toml::Value = match toml::from_str(manifest_str) {
        Ok(v) => v,
        Err(_) => return BTreeMap::new(),
    };

    let bp_table = raw
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("battery-pack"))
        .and_then(|bp| bp.as_table());

    let Some(bp_table) = bp_table else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    for (bp_name, entry) in bp_table {
        if let Some(sets) = entry.get("sets").and_then(|s| s.as_array()) {
            let set_names: Vec<String> = sets
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            result.insert(bp_name.clone(), set_names);
        }
    }

    result
}

fn parse_dep_map(raw: &BTreeMap<String, toml::Value>) -> BTreeMap<String, DepSpec> {
    let mut deps = BTreeMap::new();

    for (name, value) in raw {
        let dep = parse_single_dep(value);
        deps.insert(name.clone(), dep);
    }

    deps
}

fn parse_single_dep(value: &toml::Value) -> DepSpec {
    match value {
        toml::Value::String(version) => DepSpec {
            version: version.clone(),
            features: Vec::new(),
        },
        toml::Value::Table(table) => {
            let version = table
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let features = table
                .get("features")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            DepSpec { version, features }
        }
        _ => DepSpec {
            version: String::new(),
            features: Vec::new(),
        },
    }
}

fn parse_sets(raw: &BTreeMap<String, BTreeMap<String, toml::Value>>) -> BTreeMap<String, SetSpec> {
    let mut sets = BTreeMap::new();

    for (set_name, crates_map) in raw {
        let mut crates = BTreeMap::new();
        for (crate_name, value) in crates_map {
            let features = match value {
                toml::Value::Table(table) => table
                    .get("features")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            crates.insert(crate_name.clone(), SetCrateSpec { features });
        }
        sets.insert(set_name.clone(), SetSpec { crates });
    }

    sets
}

// ============================================================================
// Drift detection
// ============================================================================

/// Check for drift between a battery pack's spec and the user's manifest.
/// Emits `cargo:warning` for any issues found.
pub fn check_drift(bp: &BatteryPackSpec, user: &UserManifest) {
    // Find which sets the user has active for this battery pack
    let active_sets = user
        .battery_pack_sets
        .get(&bp.name)
        .cloned()
        .unwrap_or_else(|| vec!["default".to_string()]);

    let expected = bp.resolve_crates(&active_sets);

    let mut has_drift = false;

    for (crate_name, expected_dep) in &expected {
        match user.dependencies.get(crate_name) {
            None => {
                println!(
                    "cargo:warning=battery-pack({}): missing dependency '{}' (expected {})",
                    bp.name, crate_name, expected_dep.version
                );
                has_drift = true;
            }
            Some(user_dep) => {
                // Check version (simple string comparison for now — could do semver later)
                if !expected_dep.version.is_empty()
                    && !user_dep.version.is_empty()
                    && user_dep.version != expected_dep.version
                {
                    println!(
                        "cargo:warning=battery-pack({}): '{}' version is '{}', battery pack recommends '{}'",
                        bp.name, crate_name, user_dep.version, expected_dep.version
                    );
                    has_drift = true;
                }

                // Check missing features
                for feat in &expected_dep.features {
                    if !user_dep.features.contains(feat) {
                        println!(
                            "cargo:warning=battery-pack({}): '{}' is missing feature '{}'",
                            bp.name, crate_name, feat
                        );
                        has_drift = true;
                    }
                }
            }
        }
    }

    if has_drift {
        println!(
            "cargo:warning=battery-pack({}): run `cargo bp sync` to update dependencies",
            bp.name
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_battery_pack() {
        let manifest = r#"
            [package]
            name = "cli-battery-pack"
            version = "0.3.0"

            [dev-dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        assert_eq!(spec.name, "cli-battery-pack");
        assert_eq!(spec.version, "0.3.0");
        assert_eq!(spec.dev_dependencies.len(), 2);

        // All dev-deps are the default set (no explicit default)
        assert_eq!(spec.default_crates.len(), 2);
        assert!(spec.default_crates.contains(&"clap".to_string()));
        assert!(spec.default_crates.contains(&"dialoguer".to_string()));
        assert!(spec.sets.is_empty());
    }

    #[test]
    fn test_parse_with_explicit_default_and_sets() {
        let manifest = r#"
            [package]
            name = "cli-battery-pack"
            version = "0.3.0"

            [dev-dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"
            indicatif = "0.17"
            console = "0.15"

            [package.metadata.battery-pack]
            default = ["clap", "dialoguer"]

            [package.metadata.battery-pack.sets]
            indicators = { indicatif = {}, console = {} }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        assert_eq!(spec.default_crates, vec!["clap", "dialoguer"]);
        assert_eq!(spec.sets.len(), 1);

        let indicators = &spec.sets["indicators"];
        assert!(indicators.crates.contains_key("indicatif"));
        assert!(indicators.crates.contains_key("console"));
    }

    #[test]
    fn test_parse_sets_with_feature_augmentation() {
        let manifest = r#"
            [package]
            name = "async-battery-pack"
            version = "0.1.0"

            [dev-dependencies]
            tokio = { version = "1", features = ["macros", "rt"] }
            clap = { version = "4", features = ["derive"] }

            [package.metadata.battery-pack]
            default = ["clap", "tokio"]

            [package.metadata.battery-pack.sets]
            tokio-full = { tokio = { features = ["full"] } }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        let tokio_full = &spec.sets["tokio-full"];
        assert_eq!(tokio_full.crates["tokio"].features, vec!["full"]);
    }

    #[test]
    fn test_resolve_default_only() {
        let manifest = r#"
            [package]
            name = "cli-battery-pack"
            version = "0.3.0"

            [dev-dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"
            indicatif = "0.17"

            [package.metadata.battery-pack]
            default = ["clap", "dialoguer"]

            [package.metadata.battery-pack.sets]
            indicators = { indicatif = {} }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        let resolved = spec.resolve_crates(&["default".to_string()]);

        assert_eq!(resolved.len(), 2);
        assert!(resolved.contains_key("clap"));
        assert!(resolved.contains_key("dialoguer"));
        assert!(!resolved.contains_key("indicatif"));
    }

    #[test]
    fn test_resolve_with_set() {
        let manifest = r#"
            [package]
            name = "cli-battery-pack"
            version = "0.3.0"

            [dev-dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"
            indicatif = "0.17"

            [package.metadata.battery-pack]
            default = ["clap", "dialoguer"]

            [package.metadata.battery-pack.sets]
            indicators = { indicatif = {} }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        let resolved = spec.resolve_crates(&["default".to_string(), "indicators".to_string()]);

        assert_eq!(resolved.len(), 3);
        assert!(resolved.contains_key("indicatif"));
    }

    #[test]
    fn test_resolve_feature_augmentation() {
        let manifest = r#"
            [package]
            name = "async-battery-pack"
            version = "0.1.0"

            [dev-dependencies]
            tokio = { version = "1", features = ["macros", "rt"] }

            [package.metadata.battery-pack]
            default = ["tokio"]

            [package.metadata.battery-pack.sets]
            tokio-full = { tokio = { features = ["full"] } }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        let resolved = spec.resolve_crates(&["default".to_string(), "tokio-full".to_string()]);

        let tokio = &resolved["tokio"];
        assert!(tokio.features.contains(&"macros".to_string()));
        assert!(tokio.features.contains(&"rt".to_string()));
        assert!(tokio.features.contains(&"full".to_string()));
    }

    #[test]
    fn test_resolve_all() {
        let manifest = r#"
            [package]
            name = "cli-battery-pack"
            version = "0.3.0"

            [dev-dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"
            indicatif = "0.17"

            [package.metadata.battery-pack]
            default = ["clap"]

            [package.metadata.battery-pack.sets]
            indicators = { indicatif = {} }
        "#;

        let spec = parse_battery_pack(manifest).unwrap();
        let resolved = spec.resolve_all();

        // All three dev-deps should be present
        assert_eq!(resolved.len(), 3);
        assert!(resolved.contains_key("clap"));
        assert!(resolved.contains_key("dialoguer"));
        assert!(resolved.contains_key("indicatif"));
    }

    #[test]
    fn test_parse_user_manifest() {
        let manifest = r#"
            [package]
            name = "my-app"
            version = "0.1.0"

            [dependencies]
            clap = { version = "4", features = ["derive"] }
            dialoguer = "0.11"

            [build-dependencies]
            cli-battery-pack = "0.3.0"

            [package.metadata.battery-pack.cli-battery-pack]
            sets = ["default", "indicators"]
        "#;

        let user = parse_user_manifest(manifest).unwrap();
        assert_eq!(user.dependencies.len(), 2);
        assert_eq!(
            user.battery_pack_sets["cli-battery-pack"],
            vec!["default", "indicators"]
        );
    }
}
