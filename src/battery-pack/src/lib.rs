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

pub use bphelper_manifest::{BatteryPackSpec, DepSpec, assert_no_regular_deps};

/// Validate that the calling crate's dependencies match a battery pack's specs.
///
/// Call this from your battery pack's `validate()` function, passing
/// the embedded manifest string. This reads the user's Cargo.toml via
/// the runtime `CARGO_MANIFEST_DIR` env var (which, in build.rs, points
/// to the user's crate) and compares against the battery pack specs.
///
/// Emits `cargo:warning` messages for any drift. Never fails the build.
pub fn validate(self_manifest: &str) {
    let bp_spec = match bphelper_manifest::parse_battery_pack(self_manifest) {
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

    let user_manifest = match std::fs::read_to_string(&user_toml_path) {
        Ok(content) => content,
        Err(e) => {
            println!("cargo:warning=battery-pack: failed to read {user_toml_path}: {e}");
            return;
        }
    };

    let user = match bphelper_manifest::parse_user_manifest(&user_manifest) {
        Ok(u) => u,
        Err(e) => {
            println!("cargo:warning=battery-pack: failed to parse user manifest: {e}");
            return;
        }
    };

    bphelper_manifest::check_drift(&bp_spec, &user);

    // Rerun validation when user's Cargo.toml changes
    println!("cargo:rerun-if-changed={user_toml_path}");
}
