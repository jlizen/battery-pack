const SELF_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

/// Validate that the calling crate's dependencies match this battery pack's specs.
///
/// Call this from your build.rs:
/// ```rust,ignore
/// fn main() {
///     error_battery_pack::validate();
/// }
/// ```
pub fn validate() {
    battery_pack::validate(SELF_MANIFEST);
}

#[cfg(test)]
mod tests {
    #[test]
    fn battery_pack_has_no_regular_dependencies() {
        battery_pack::assert_no_regular_deps(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/Cargo.toml"
        )));
    }
}
