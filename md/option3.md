# Option 3: Sync-Based Battery Packs with Sets

This document evolves option2's sync-based approach with a sets model for organizing battery pack contents.

## Core Idea

A battery pack is defined by its dev-dependencies. All curated crates live there — with versions, features, everything. Named **sets** provide grouping and feature augmentation. The CLI syncs real dependencies into the user's Cargo.toml.

Proc macros just work because crates are real dependencies, not re-exports.

## Defining a Battery Pack

### Minimal (all crates are default)

```toml
[package]
name = "cli-battery-pack"
version = "0.3.0"

[dev-dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"
```

No metadata needed. All four crates are the default set.

### With a curated default and named sets

```toml
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
```

Default gives you clap + dialoguer. The `indicators` set adds indicatif + console.

### Sets that augment features

Sets can add new crates, add features to existing crates, or both:

```toml
[dev-dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt"] }
indicatif = "0.17"
console = "0.15"

[package.metadata.battery-pack]
default = ["clap", "tokio"]

[package.metadata.battery-pack.sets]
tokio-full = { tokio = { features = ["full"] } }
indicators = { indicatif = {}, console = {} }
fancy-cli = { clap = { features = ["color"] }, indicatif = {}, console = {} }
```

- `tokio-full` doesn't add crates, just augments tokio with the `full` feature
- `indicators` adds two crates
- `fancy-cli` adds two crates *and* augments clap with `color`

Feature merging is always **additive** — enabling a set unions features with the base, never removes.

## CLI Commands

### `cargo bp add cli`

Syncs the default set into your Cargo.toml:

1. Adds `cli-battery-pack` to `[build-dependencies]`
2. Adds default set crates to `[dependencies]` (with versions and features from dev-deps)
3. Creates/modifies `build.rs` to call `cli_battery_pack::validate()`

### `cargo bp add cli --with indicators`

Syncs the default set plus the named set(s).

### `cargo bp add cli --all`

Syncs every dev-dependency regardless of what `default` says.

### `cargo bp sync`

Updates an existing battery pack installation:

- Bumps versions (only if current < battery pack version)
- Adds features the battery pack has added since last sync
- Adds new crates if they've been added to your active sets
- Warns about missing required crates

### `cargo bp enable indicators`

Activates a set after initial install:

- Finds which battery pack(s) define the `indicators` set
- Syncs those crates/features into `[dependencies]`

## User's Cargo.toml After Install

After `cargo bp add cli --with indicators`:

```toml
[build-dependencies]
cli-battery-pack = "0.3.0"

[dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"

[package.metadata.battery-pack.cli-battery-pack]
sets = ["default", "indicators"]
```

The metadata section tracks which sets are active, so `cargo bp sync` knows what to maintain.

## What the User Writes

```rust
use clap::Parser;
use dialoguer::Input;
use indicatif::ProgressBar;

#[derive(Parser)]  // proc macros just work
struct Cli {
    #[arg(short, long)]
    name: String,
}
```

No namespacing. Real crates, real dependencies.

## Battery Pack Invariants

Battery packs must only have dev-dependencies — any regular `[dependencies]` would leak into users' dependency trees for no reason. This is enforced by a generated test that `cargo bp new` includes in the template:

```rust
#[test]
fn battery_pack_has_no_regular_dependencies() {
    let manifest: toml::Value = toml::from_str(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
    ).unwrap();
    assert!(
        manifest.get("dependencies").is_none(),
        "Battery packs must only use [dev-dependencies], not [dependencies]"
    );
}
```

This catches violations during `cargo test`, which authors run before publishing.

## Build-Time Validation

The battery pack embeds its own Cargo.toml at compile time using `include_str!` and `CARGO_MANIFEST_DIR`. No build.rs on the battery pack side — just lib.rs:

```rust
// cli-battery-pack/src/lib.rs

/// The battery pack's own manifest, embedded at compile time.
/// `CARGO_MANIFEST_DIR` here resolves to the battery pack's directory,
/// giving us access to the dev-dependencies and metadata specs.
const SELF_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

/// Validate that the calling crate's dependencies match this battery pack's specs.
///
/// Call this from your build.rs. It reads the user's Cargo.toml via the
/// runtime `CARGO_MANIFEST_DIR` env var (which, in build.rs, points to
/// the user's crate) and compares against the battery pack specs embedded
/// in `SELF_MANIFEST`.
pub fn validate() {
    let bp_spec = parse_battery_pack(SELF_MANIFEST);

    let user_toml_path = format!(
        "{}/Cargo.toml",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    let user_manifest = std::fs::read_to_string(&user_toml_path).unwrap();

    // Compare user's deps against bp_spec
    // Emit cargo:warning for any drift
}
```

The key trick: `env!("CARGO_MANIFEST_DIR")` in the `const` is evaluated at **compile time** of the battery pack crate, so it captures the battery pack's own Cargo.toml. But `std::env::var("CARGO_MANIFEST_DIR")` inside `validate()` is evaluated at **runtime** (inside the user's build.rs), so it points to the user's crate. We naturally get both manifests without any path trickery.

The user's `build.rs` calls it:

```rust
fn main() {
    cli_battery_pack::validate();
}
```

Every build, `validate()`:
- Parses the embedded battery pack manifest for dev-deps, default set, and named sets
- Reads the user's Cargo.toml and active sets from `[package.metadata.battery-pack.cli-battery-pack]`
- Warns (never fails) if deps are missing, outdated, or feature-incomplete
- Suggests `cargo bp sync` if drift is detected

## Multiple Battery Packs

```toml
[build-dependencies]
cli-battery-pack = "0.3.0"
async-battery-pack = "0.2.0"

[dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[package.metadata.battery-pack.cli-battery-pack]
sets = ["default"]

[package.metadata.battery-pack.async-battery-pack]
sets = ["default", "tokio-full"]
```

Each battery pack tracks its own active sets. Validate calls chain:

```rust
fn main() {
    cli_battery_pack::validate();
    async_battery_pack::validate();
}
```

## Default Set Rules

1. If no `default` in metadata → all dev-dependencies are the default set
2. If `default` is specified → only those crates are the default set
3. `--all` always syncs everything regardless of `default`

## Open Questions

### Workspace-level dependencies

Should `cargo bp add` inject into `[workspace.dependencies]` when a workspace exists? Could detect workspace presence and behave accordingly.

### Feature conflicts

When two battery packs recommend different features for the same crate (e.g., tokio), the merge is additive. But should `cargo bp sync` warn about this?

### Removing sets

What does `cargo bp disable indicators` do? Remove the crates? Only if no other battery pack needs them? Just remove the set from the active list and let validate warn?

## Comparison to Option 2

| Aspect | Option 2 | Option 3 |
|--------|----------|----------|
| Optional deps | `disabled` section per crate | Named sets |
| Granularity | Individual crate enable/disable | Groups of crates |
| Feature augmentation | Not supported | Sets can add features |
| Battery pack authoring | Dev-deps + metadata for optionals | Just dev-deps (minimal case) |
| Default | Required vs optional distinction | Everything unless narrowed |
