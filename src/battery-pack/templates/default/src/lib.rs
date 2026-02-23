const SELF_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

/// Validate that the calling crate's dependencies match this battery pack's specs.
///
/// Call this from your build.rs:
/// ```rust,ignore
/// fn main() {
///     {{crate_name}}::validate();
/// }
/// ```
pub fn validate() {
    battery_pack::validate(SELF_MANIFEST);
}

#[cfg(test)]
mod tests {
    #[test]
    fn battery_pack_has_no_regular_dependencies() {
        let manifest: toml::Value = toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/Cargo.toml"
        )))
        .unwrap();
        let deps = manifest.get("dependencies").and_then(|d| d.as_table());
        let non_bp_deps: Vec<_> = deps
            .iter()
            .flat_map(|d| d.keys())
            .filter(|k| *k != "battery-pack")
            .collect();
        assert!(
            non_bp_deps.is_empty(),
            "Battery packs must only use [dev-dependencies] (plus battery-pack). Found: {:?}",
            non_bp_deps
        );
    }
}
