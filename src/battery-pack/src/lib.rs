//! battery-pack: Framework for building and using battery packs.
//!
//! Battery packs are curated collections of crates that work well together.
//! The CLI (`cargo bp`) syncs real dependencies into your Cargo.toml,
//! and this library provides build-time validation to detect drift.
//!
//! # For Battery Pack Authors
//!
//! Your lib.rs should look like this:
//!
//! ```rust,ignore
//! const SELF_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
//!
//! pub fn validate() {
//!     battery_pack::validate(SELF_MANIFEST);
//! }
//! ```
//!
//! # For Battery Pack Users
//!
//! Your build.rs should call validate:
//!
//! ```rust,ignore
//! fn main() {
//!     cli_battery_pack::validate();
//! }
//! ```

pub use bphelper_manifest::{BatteryPackSpec, CrateSpec, DepKind};

/// Validate that the calling crate's dependencies match a battery pack's specs.
///
/// Call this from your battery pack's `validate()` function, passing
/// the embedded manifest string. This reads the user's Cargo.toml via
/// the runtime `CARGO_MANIFEST_DIR` env var (which, in build.rs, points
/// to the user's crate) and compares against the battery pack specs.
///
/// Emits `cargo:warning` messages for any drift. Never fails the build.
pub fn validate(self_manifest: &str) {
    let _bp_spec = match bphelper_manifest::parse_battery_pack(self_manifest) {
        Ok(spec) => spec,
        Err(e) => {
            println!("cargo:warning=battery-pack: failed to parse battery pack manifest: {e}");
            return;
        }
    };

    let user_toml_path = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => format!("{dir}/Cargo.toml"),
        Err(_) => {
            println!("cargo:warning=battery-pack: CARGO_MANIFEST_DIR not set, skipping validation");
            return;
        }
    };

    // TODO: implement drift detection against user's Cargo.toml
    // For now, just ensure we rerun when the user's manifest changes.
    println!("cargo:rerun-if-changed={user_toml_path}");
}
