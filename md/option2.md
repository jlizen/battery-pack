# Option 2: Sync-Based Battery Packs

This document describes an alternative design for battery packs that addresses the proc-macro path resolution problem inherent in the facade approach.

## The Problem

The current facade approach re-exports crates through a battery pack namespace:

```rust
use cli::clap::Parser;
```

This breaks proc macros that emit code with absolute paths like `::serde::Serialize`. The proc macro assumes `serde` is a direct dependency, but in the facade model only `cli-battery-pack` is in scope at the crate root.

## The Solution: Sync Instead of Facade

Instead of re-exporting crates, battery packs become **dependency specifications** that sync real dependencies into your Cargo.toml. The battery pack crate itself becomes a build-dependency that validates your setup.

### User's Cargo.toml After `cargo bp add cli`

```toml
[build-dependencies]
cli-battery-pack = "0.3.0"

[dependencies]
clap = "4"
dialoguer = "0.11"

[package.metadata.cli-battery-pack]
disabled = [
    { indicatif = "0.17" },
    { console = "0.15" },
]
```

### What the user writes

```rust
use clap::Parser;
use dialoguer::Input;

#[derive(Parser)]  // proc macros just work!
struct Cli {
    #[arg(short, long)]
    name: String,
}
```

No namespace prefixing. Real crates as real dependencies.

## How It Works

### Battery Pack Structure

A battery pack declares its curated crates as **dev-dependencies**:

```toml
# cli-battery-pack/Cargo.toml

[package]
name = "cli-battery-pack"
version = "0.3.0"

[dev-dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = { version = "0.17", optional-for-users = true }
console = { version = "0.15", optional-for-users = true }
```

At build time, the battery pack's `build.rs` reads these dev-dependencies and generates a `validate()` function with the specs baked in:

```rust
// generated in cli-battery-pack at build time
pub fn validate() {
    const REQUIRED: &[(&str, &str)] = &[
        ("clap", "4"),
        ("dialoguer", "0.11"),
    ];
    const OPTIONAL: &[(&str, &str)] = &[
        ("indicatif", "0.17"),
        ("console", "0.15"),
    ];

    // read CARGO_MANIFEST_DIR/Cargo.toml
    // compare against expected deps
    // emit cargo:warning if sync needed
}
```

### CLI Commands

**`cargo bp add cli`**
1. Adds `cli-battery-pack = "0.3.0"` to `[build-dependencies]`
2. Adds required deps to `[dependencies]`
3. Adds optional deps to `[package.metadata.cli-battery-pack].disabled`
4. Creates/modifies `build.rs` to call `cli_battery_pack::validate()`

**`cargo bp sync`**
- Bumps versions in `[dependencies]` and `disabled` (only if current version < battery pack version)
- Adds new optionals to `disabled`
- Removes stale entries from `disabled`
- Never adds to `[dependencies]` except version bumps for things already there
- Issues warnings for missing required dependencies

**`cargo bp enable indicatif`**
- Finds which battery pack(s) have `indicatif` in their `disabled` list
- Moves the dependency from `disabled` to `[dependencies]`
- If multiple battery packs have it, prompts or errors

### Build-Time Validation

The user's `build.rs` calls validate:

```rust
fn main() {
    cli_battery_pack::validate();

    // ... rest of user's build.rs if any
}
```

Every build, `validate()`:
- Reads the user's Cargo.toml via `CARGO_MANIFEST_DIR`
- Compares against baked-in required/optional specs
- If deps are missing or outdated → `cargo:warning=run cargo bp sync`
- Never fails the build, just warns

## Adding build.rs

When `cargo bp add` runs:

- If no `build.rs` exists: creates one with `cli_battery_pack::validate()` in main
- If `build.rs` exists: searches for `fn main` and inserts the validate call at the beginning
- If modification fails: instructs the user to add it manually

## Multiple Battery Packs

A crate can use multiple battery packs:

```toml
[build-dependencies]
cli-battery-pack = "0.3.0"
async-battery-pack = "0.2.0"
error-battery-pack = "0.1.0"

[dependencies]
clap = "4"
tokio = { version = "1", features = ["full"] }
anyhow = "1"

[package.metadata.cli-battery-pack]
disabled = [{ indicatif = "0.17" }]

[package.metadata.async-battery-pack]
disabled = [{ async-std = "1.12" }]
```

Each battery pack has its own metadata section. The validate calls can be chained:

```rust
fn main() {
    cli_battery_pack::validate();
    async_battery_pack::validate();
    error_battery_pack::validate();
}
```

## Open Questions

### Workspace-Level Dependencies

Should battery packs inject at the workspace level instead?

```toml
# Workspace Cargo.toml
[workspace.dependencies]
clap = "4"
dialoguer = "0.11"
```

```toml
# Package Cargo.toml
[dependencies]
clap.workspace = true
dialoguer.workspace = true
```

Pros:
- Centralized version management
- Consistent versions across workspace members
- Closer to how many projects already organize

Cons:
- More complex for single-crate projects
- Two places to look/modify
- Workspace might not exist

Possible approach: detect workspace presence and behave accordingly, or make it configurable.

### Feature Alignment

When a battery pack recommends features (e.g., `clap` with `derive`), how do we handle:
- User already has `clap` without `derive`
- User wants different features than recommended

Possible: `cargo bp sync` warns about feature mismatches but doesn't overwrite user choices.

### Marking Optional Dependencies

The `optional-for-users = true` syntax shown above isn't valid Cargo.toml. We need a way for battery pack authors to mark which dev-dependencies are optional for users. Options:
- Custom metadata section in the battery pack
- Naming convention
- Separate `[optional-for-users]` section

## Comparison to Facade Approach

| Aspect | Facade | Sync |
|--------|--------|------|
| Proc macros | Broken | Work |
| User's Cargo.toml | Clean (one dep) | More dependencies |
| Namespacing | `cli::clap` | `clap` directly |
| Version control | Automatic | Requires sync |
| Customization | Limited | Full control |
| Understanding | Magic | Transparent |

## Migration Path

For existing users of facade-style battery packs:
1. `cargo bp migrate` could convert facade deps to synced deps
2. Update use statements from `cli::clap` to `clap`
3. Update Cargo.toml structure

The CLI can detect old-style usage and prompt for migration.
