# Option 3: Sync-Based Battery Packs with Sets

This document evolves option2's sync-based approach with a sets model for organizing battery pack contents.

## Core Idea

A battery pack is defined by its dev-dependencies. All curated crates live there — with versions, features, everything. Named **sets** provide grouping and feature augmentation. The CLI syncs real dependencies into the user's Cargo.toml.

Proc macros just work because crates are real dependencies, not re-exports.

The previous facade approach (generating `pub use` re-exports) is fully replaced — no transition period, no feature flags.

## Defining a Battery Pack

### Minimal (all crates are default)

```toml
[package]
name = "cli-battery-pack"
version = "0.3.0"

[dependencies]
battery-pack = "0.3"

[dev-dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"
```

No metadata needed. All four dev-deps are the default set. The only regular dependency is `battery-pack` itself (for the validation function).

### With a curated default and named sets

```toml
[package]
name = "cli-battery-pack"
version = "0.3.0"

[dependencies]
battery-pack = "0.3"

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

## Battery Pack lib.rs

Each battery pack's lib.rs is minimal — it embeds its own manifest and delegates to the shared validation function in the `battery-pack` crate:

```rust
// cli-battery-pack/src/lib.rs

const SELF_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

pub fn validate() {
    battery_pack::validate(SELF_MANIFEST);
}
```

There is no build.rs on the battery pack side. No facade generation. The `battery-pack` crate provides the shared `validate()` implementation that parses the embedded manifest, reads the user's Cargo.toml, and emits warnings.

## CLI Commands

### `cargo bp add cli`

Syncs the default set into your Cargo.toml:

1. Resolves `cli` to `cli-battery-pack`
2. Downloads the crate tarball from crates.io, reads its Cargo.toml
3. Parses dev-dependencies, sets, and default configuration
4. Adds `cli-battery-pack` to `[build-dependencies]`
5. Adds default set crates to `[dependencies]` (with versions and features from dev-deps)
6. Records active sets in `[package.metadata.battery-pack.cli-battery-pack]`
7. Creates/modifies `build.rs` to call `cli_battery_pack::validate()`

### `cargo bp add cli --with indicators`

Syncs the default set plus the named set(s).

### `cargo bp add cli --all`

Syncs every dev-dependency regardless of what `default` says.

### `cargo bp sync`

Updates an existing battery pack installation:

- Reads all battery packs from `[build-dependencies]`
- Downloads each one's tarball and parses its spec
- For each, checks the user's active sets and compares against `[dependencies]`
- Bumps versions (only if current < battery pack version)
- Adds features the battery pack has added since last sync
- Adds new crates if they've been added to active sets

### `cargo bp enable [battery-pack] <set-name>`

Activates a set after initial install:

- If battery-pack name given, searches that one; otherwise searches all installed battery packs (found via `[build-dependencies]`)
- Downloads/parses the battery pack's spec
- Verifies the set exists
- Adds the set to `[package.metadata.battery-pack.<name>].sets`
- Syncs the set's crates/features into `[dependencies]`

### Workspace-level dependencies

When the user's crate is part of a workspace, `cargo bp add` and `cargo bp sync` inject dependencies into `[workspace.dependencies]` in the workspace root Cargo.toml and reference them as `crate.workspace = true` in the crate's `[dependencies]`. If no workspace exists, dependencies are added directly to the crate's `[dependencies]` with full version/features.

### build.rs modification

Creating a new `build.rs` is straightforward. Modifying an existing one:

1. Parse the file with `syn` to locate the `fn main()` function
2. Use string manipulation to append the `validate()` call (not AST reconstruction)
3. Check if the call is already present before adding

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

Battery packs must only have dev-dependencies (plus `battery-pack` itself as the sole regular dependency). This is enforced by a generated test that `cargo bp new` includes in the template:

```rust
#[test]
fn battery_pack_has_no_regular_dependencies() {
    let manifest: toml::Value = toml::from_str(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
    ).unwrap();
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
```

## Build-Time Validation

The `battery-pack` crate provides a shared validation function:

```rust
// battery-pack/src/lib.rs

pub fn validate(self_manifest: &str) {
    let bp_spec = parse_battery_pack(self_manifest);

    let user_toml_path = format!(
        "{}/Cargo.toml",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    let user_manifest = std::fs::read_to_string(&user_toml_path).unwrap();

    // Parse user's deps and active sets
    // Compare against bp_spec
    // Emit cargo:warning for any drift
    // Suggest `cargo bp sync` if drift detected
}
```

The key trick: `env!("CARGO_MANIFEST_DIR")` in the battery pack's `const SELF_MANIFEST` is evaluated at **compile time** of the battery pack crate, so it captures the battery pack's own Cargo.toml. But `std::env::var("CARGO_MANIFEST_DIR")` inside `validate()` is evaluated at **runtime** (inside the user's build.rs), so it points to the user's crate. We naturally get both manifests without any path trickery.

The user's `build.rs` calls it:

```rust
fn main() {
    cli_battery_pack::validate();
}
```

Every build, `validate()`:
- Parses the embedded battery pack manifest for dev-deps, default set, and named sets
- Reads the user's Cargo.toml and active sets from `[package.metadata.battery-pack.cli-battery-pack]`
- Warns (never fails) if deps are missing, outdated, or feature-incomplete — output is `cargo:warning` messages only
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

When two battery packs recommend different features for the same crate, the merge is additive (features are unioned). This is correct behavior, not a conflict.

## Default Set Rules

1. If no `default` in metadata -> all dev-dependencies are the default set
2. If `default` is specified -> only those crates are the default set
3. `--all` always syncs everything regardless of `default`

## Deferred

- **`cargo bp disable`** — removing sets and their crates. Semantics TBD (remove crates? only if no other battery pack needs them? just remove from active list?).

## Comparison to Option 2

| Aspect | Option 2 | Option 3 |
|--------|----------|----------|
| Optional deps | `disabled` section per crate | Named sets |
| Granularity | Individual crate enable/disable | Groups of crates |
| Feature augmentation | Not supported | Sets can add features |
| Battery pack authoring | Dev-deps + metadata for optionals | Just dev-deps (minimal case) |
| Default | Required vs optional distinction | Everything unless narrowed |

## Implementation Plan

### Step 0: Update this design doc
- [x] Incorporate Q&A clarifications
- [x] Resolve open questions (workspace: yes, disable: deferred, validation: cargo:warning only)
- [x] Add implementation plan section

### Step 1: Restructure the `battery-pack` crate
- [x] Delete `bphelper-build/` entirely (facade generation is gone)
- [x] Create `bphelper-manifest/` shared crate for manifest parsing (avoids circular dep between battery-pack lib and CLI)
- [x] Update workspace Cargo.toml (remove bphelper-build, add bphelper-manifest)
- [x] Update `battery-pack` Cargo.toml (depend on bphelper-manifest)
- [x] Update `battery-pack/src/lib.rs` (remove facade re-export, add validate via bphelper-manifest)

### Step 2: Implement manifest parsing
- [x] Parse `[dev-dependencies]` into structured types
- [x] Parse `[package.metadata.battery-pack]` (default, sets)
- [x] Resolve active sets into merged crate list with versions and features
- [x] Unit tests for parsing

### Step 3: Implement `validate()`
- [x] Read user's Cargo.toml and active sets
- [x] Compare against battery pack spec
- [x] Emit `cargo:warning` for drift
- [x] Unit tests for validation logic

### Step 4: Rewrite `cargo bp add`
- [x] Download tarball (or read local path), parse battery pack spec
- [x] Add to `[build-dependencies]`
- [x] Sync crates to `[dependencies]` (with workspace detection)
- [x] Record active sets in metadata
- [x] Create/modify build.rs (parse with syn, edit with string manipulation)
- [x] Update CLI arguments (replace --features with --with and --all)

### Step 5: Implement `cargo bp sync`
- [x] Read installed battery packs from `[build-dependencies]`
- [x] Download/parse each, compare against user's deps
- [x] Bump versions, add features, add crates as needed

### Step 6: Implement `cargo bp enable`
- [x] Accept optional battery-pack name + required set name
- [x] Search installed battery packs if name not given
- [x] Add set to active list and sync its crates

### Step 7: Update `cargo bp new` template
- [x] Cargo.toml with `[dev-dependencies]` and `battery-pack` as regular dep
- [x] lib.rs with `SELF_MANIFEST` const and `validate()` wrapper
- [x] No build.rs on battery pack side
- [x] Include no-regular-deps test

### Step 8: Delete and recreate example battery packs
- [x] Delete cli-battery-pack, error-battery-pack, logging-battery-pack
- [ ] Recreate each using `cargo bp new` as integration test of tooling
