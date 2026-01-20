# Battery Pack: A Curation Layer for the Rust Ecosystem

## Overview

A **Battery Pack** is a curated collection of Rust crates that work well together, bundled with documentation, templates, AI skills, and cross-cutting tests. It solves the discoverability and "getting started" problems in the Rust ecosystem.

A Battery Pack is:

1. **A facade crate** — re-exports curated crates (users interact with the crates directly)
2. **A runnable binary** — CLI for interacting with the pack (init, upgrade, docs, etc.)
3. **Templates** — cargo-generate templates for common starting points
4. **Skills** — machine-readable guidance for AI agents and humans
5. **Cross-cutting tests** — verify the crates work together

**Philosophy:** A battery pack is lightweight curation, not abstraction. It's a collection of crates that work well together. Users interact directly with the underlying crates—the battery pack just makes them easier to discover and get started with. Think of it as "batteries included" for a domain, not a framework that wraps everything.

The `battery-pack` crate is itself a "battery pack for battery packs" — it provides the framework for building battery packs.

## Design Goals

- **Build on cargo-generate** — leverage existing template infrastructure
- **Convention over configuration** — sensible defaults, escape hatches when needed
- **Discoverable via crates.io** — use keywords and metadata, no new registry
- **Runnable binaries** — each battery pack is its own CLI
- **Extensible** — pack authors can add custom commands and behavior
- **Lifecycle-aware** — support upgrades and migrations between versions

## Crate Structure

### A Battery Pack Crate

```
web-battery/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Facade: re-exports from dependencies
│   └── main.rs         # CLI binary
├── templates/
│   ├── server/         # cargo-generate template
│   ├── api/            # cargo-generate template
│   └── ...
├── skills/
│   ├── async-patterns.md
│   ├── error-handling.md
│   └── ...
├── tests/
│   └── cross-cutting/  # Integration tests across the crate ecosystem
└── docs/
    └── guide.md        # How these crates fit together
```

### Cargo.toml Metadata

```toml
[package]
name = "web-battery"
version = "1.0.0"
keywords = ["battery-pack", "web", "async", "http"]

[package.metadata.battery]
schema_version = 1

[package.metadata.battery.templates]
server = { path = "templates/server", description = "Async web server with Tower middleware" }
api = { path = "templates/api", description = "REST API with OpenAPI docs" }
minimal = { path = "templates/minimal", description = "Minimal async binary" }

[package.metadata.battery.modules]
http = ["reqwest", "tower", "hyper"]
async = ["tokio"]
serialization = ["serde", "serde_json"]
database = ["sqlx"]

[package.metadata.battery.skills]
path = "skills/"

[lib]
# The facade

[[bin]]
name = "web-battery"
path = "src/main.rs"

[dependencies]
battery-pack = "0.1"  # Framework
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"
tower = "0.4"
# ... etc
```

## Facade Generation

Battery packs re-export their dependency crates so users can access them through a single dependency. Instead of hand-writing these re-exports, a build script generates them from Cargo.toml metadata.

The default is simple: re-export each crate at the root level. Users then use the crates directly (e.g., `web_battery::tokio::spawn()`). More complex organization is available but not the norm.

### build.rs

```rust
// web-battery/build.rs
fn main() {
    battery_pack::build::generate_facade().unwrap();
}
```

### lib.rs

```rust
// web-battery/src/lib.rs
include!(concat!(env!("OUT_DIR"), "/facade.rs"));

// Any hand-written additions, glue code, or overrides go here
```

### Metadata Levels

**No metadata (default)**

All dependency crates are re-exported at the root level.

```toml
[package.metadata.battery]
schema_version = 1

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"
serde = { version = "1", features = ["derive"] }
```

Generates:

```rust
pub use tokio;
pub use reqwest;
pub use serde;
```

Users access them as `my_battery::tokio`, `my_battery::reqwest`, etc.

### Root Exports

**Crate re-export at root (default and typical):**

```toml
[package.metadata.battery]
schema_version = 1
root = ["tokio", "serde"]
```

Generates:

```rust
pub use tokio;
pub use serde;
```

**Glob re-export (less common):**

```toml
[package.metadata.battery]
schema_version = 1
root = { tokio = "*", serde = "*" }
```

Generates:

```rust
pub use tokio::*;
pub use serde::*;
```

**Explicit items at root (rare, for convenience re-exports):**

```toml
[package.metadata.battery]
schema_version = 1

[package.metadata.battery.root]
tokio = ["spawn", "select", "main", "test"]
serde = ["Serialize", "Deserialize"]
```

Generates:

```rust
pub use tokio::{spawn, select, main, test};
pub use serde::{Serialize, Deserialize};
```

### Module Exports

For organizing crates into logical groups (optional):

**Crate re-export in modules (typical if using modules):**

```toml
[package.metadata.battery]
schema_version = 1

[package.metadata.battery.modules]
http = ["reqwest", "tower", "hyper"]
async = ["tokio"]
serialization = ["serde", "serde_json"]
```

Generates:

```rust
pub mod http {
    pub use reqwest;
    pub use tower;
    pub use hyper;
}

pub mod r#async {
    pub use tokio;
}

pub mod serialization {
    pub use serde;
    pub use serde_json;
}
```

**Glob re-export in modules (less common):**

```toml
[package.metadata.battery]
schema_version = 1

[package.metadata.battery.modules]
http = { reqwest = "*", tower = "*" }
```

Generates:

```rust
pub mod http {
    pub use reqwest::*;
    pub use tower::*;
}
```

**Explicit items in modules (rare):**

```toml
[package.metadata.battery]
schema_version = 1

[package.metadata.battery.modules.http]
reqwest = ["Client", "Response", "Error"]
tower = ["Service", "ServiceExt", "Layer"]
hyper = ["Body", "Request", "Response as HyperResponse"]

[package.metadata.battery.modules.serialization]
serde = ["Serialize", "Deserialize"]
serde_json = ["json", "Value", "from_str", "to_string"]
```

Generates:

```rust
pub mod http {
    pub use reqwest::{Client, Response, Error};
    pub use tower::{Service, ServiceExt, Layer};
    pub use hyper::{Body, Request, Response as HyperResponse};
}

pub mod serialization {
    pub use serde::{Serialize, Deserialize};
    pub use serde_json::{json, Value, from_str, to_string};
}
```

### Summary

| Config | Result |
|--------|--------|
| No metadata | All deps → `pub use dep;` at root |
| `root = ["tokio"]` | `pub use tokio;` at root |
| `root = { tokio = "*" }` | `pub use tokio::*;` at root |
| `[root] tokio = ["spawn"]` | `pub use tokio::spawn;` at root |
| `modules.http = ["reqwest"]` | `pub mod http { pub use reqwest; }` |
| `modules.http = { reqwest = "*" }` | `pub mod http { pub use reqwest::*; }` |
| `[modules.http] reqwest = ["Client"]` | `pub mod http { pub use reqwest::Client; }` |

The default is crate-level re-exports (`pub use crate_name;`). Users access the crates directly through the battery pack namespace. Glob re-exports and item-level exports are available for packs that want different ergonomics.

### Mixed Approach

Combine root and module exports freely:

```toml
[package.metadata.battery]
schema_version = 1
root = ["tokio", "serde"]

[package.metadata.battery.modules]
http = ["reqwest", "tower"]
```

Generates:

```rust
// Root level
pub use tokio;
pub use serde;

// Modules
pub mod http {
    pub use reqwest;
    pub use tower;
}
```

Any dependency not mentioned in `root`, `[root]`, or `modules` is **not exported** (once you start configuring explicitly).

### Excluding Dependencies

Some dependencies (like `battery-pack` itself, or build-only deps) shouldn't be re-exported:

```toml
[package.metadata.battery]
schema_version = 1
exclude = ["battery-pack", "syn", "quote"]  # Don't re-export these
```

## The `battery-pack` Crate

### Library API

```rust
// battery-pack/src/lib.rs

pub use cargo_generate;  // Re-export for battery authors

/// Metadata about a battery pack, derived from Cargo.toml
pub struct BatteryPackInfo {
    pub name: String,
    pub version: Version,
    pub templates: Vec<Template>,
    pub modules: Vec<Module>,
    pub skills_path: Option<PathBuf>,
}

/// A template available in the battery pack
pub struct Template {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    pub git: Option<String>,  // Alternative: external git repo
}

/// A logical grouping of crates
pub struct Module {
    pub name: String,
    pub crates: Vec<String>,
}

/// Trait for battery pack implementations
pub trait BatteryPack: Sized {
    fn info() -> BatteryPackInfo;
    
    // Override points for custom behavior
    fn pre_init(&self, args: &InitArgs) -> Result<()> { Ok(()) }
    fn post_init(&self, args: &InitArgs, project_path: &Path) -> Result<()> { Ok(()) }
    fn pre_upgrade(&self, args: &UpgradeArgs) -> Result<()> { Ok(()) }
    fn post_upgrade(&self, args: &UpgradeArgs) -> Result<()> { Ok(()) }
}

/// Derive macro for simple cases
pub use battery_pack_macros::BatteryPack;

/// Standard CLI commands
#[derive(clap::Subcommand)]
pub enum StandardCommands {
    /// Initialize a new project from a template
    Init(InitArgs),
    /// Upgrade an existing project to a new battery pack version
    Upgrade(UpgradeArgs),
    /// Show documentation
    Docs(DocsArgs),
    /// Run cross-cutting tests
    Test(TestArgs),
    /// List available templates
    Templates,
    /// Show skills for AI agents
    Skills(SkillsArgs),
}

#[derive(clap::Args)]
pub struct InitArgs {
    /// Project name
    pub name: String,
    /// Template to use
    #[arg(short, long)]
    pub template: Option<String>,
    /// Additional cargo-generate defines
    #[arg(short, long)]
    pub define: Vec<String>,
    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}
```

### The Main Macro

For the simplest case:

```rust
// web-battery/src/main.rs
use battery_pack::prelude::*;

#[derive(BatteryPack)]
struct WebBattery;

#[battery_pack::main]
fn main(command: Command<WebBattery>) -> anyhow::Result<()> {
    command.run()
}
```

With extensions:

```rust
use battery_pack::prelude::*;
use clap::Subcommand;

#[derive(BatteryPack)]
struct WebBattery;

#[derive(Subcommand)]
enum WebCommands {
    /// Add a new route to an existing project
    AddRoute {
        #[arg(short, long)]
        path: String,
        #[arg(short, long, default_value = "GET")]
        method: String,
    },
    /// Deploy to a cloud provider
    Deploy {
        #[arg(long)]
        environment: String,
    },
}

#[battery_pack::main]
fn main(command: Command<WebBattery, WebCommands>) -> anyhow::Result<()> {
    match command {
        Command::Standard(cmd) => cmd.run(),
        Command::Extension(WebCommands::AddRoute { path, method }) => {
            println!("Adding route: {} {}", method, path);
            // Custom implementation
            Ok(())
        }
        Command::Extension(WebCommands::Deploy { environment }) => {
            println!("Deploying to {}", environment);
            // Custom implementation
            Ok(())
        }
    }
}
```

Full control (no macro):

```rust
use battery_pack::prelude::*;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: MyCommands,
}

#[derive(Subcommand)]
enum MyCommands {
    // Cherry-pick what you want
    Init(battery_pack::InitArgs),
    // Add your own
    CustomThing { /* ... */ },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        MyCommands::Init(args) => {
            battery_pack::commands::init::<WebBattery>(args)
        }
        MyCommands::CustomThing { /* ... */ } => {
            // Your code
            Ok(())
        }
    }
}
```

## CLI Usage

### End Users

```bash
# Install a battery pack
cargo install web-battery

# See available templates
web-battery templates

# Initialize a new project
web-battery init my-app --template server

# Or with cargo battery dispatcher
cargo install battery-pack
cargo battery web init my-app --template server

# Upgrade an existing project
cd my-app
web-battery upgrade

# View docs
web-battery docs

# View skills (for AI agents or curious humans)
web-battery skills async-patterns
```

### Battery Pack Authors

```bash
# Bootstrap a new battery pack
cargo battery init my-battery --template battery-pack

# Test your battery pack
cd my-battery
cargo test
cargo run -- templates  # Test CLI
```

## Template Integration

Templates are standard cargo-generate templates. The battery pack adds:

1. **Template discovery** — listed in `[package.metadata.battery.templates]`
2. **Automatic variables** — `battery_pack_name`, `battery_pack_version` injected
3. **Post-generation hooks** — write metadata to generated project for upgrades

### Generated Project Metadata

After `web-battery init`, the generated project includes:

```toml
# In the generated Cargo.toml
[package.metadata.battery-generated]
pack = "web-battery"
pack_version = "1.0.0"
template = "server"
generated_at = "2024-01-15T10:30:00Z"
```

This enables `web-battery upgrade` to know what version/template was used.

## Upgrade Path

```bash
# In a project generated by web-battery 1.0
web-battery upgrade

# Output:
# Current: web-battery 1.0.0 (template: server)
# Available: web-battery 2.0.0
# 
# Changes in 2.0:
# - tokio updated to 1.35 (was 1.32)
# - Added tower-http for CORS support
# - Template changes: see migration guide
#
# Migration guide: https://...
# 
# Run with --apply to update Cargo.toml dependencies
```

## Discovery

Battery packs are discoverable via:

1. **crates.io keyword**: `battery-pack`
2. **cargo battery search**: `cargo battery search web`
3. **Central registry/list** (optional): curated list of known battery packs

```bash
cargo battery search web
# Results:
#   web-battery      1.0.0  Async web development with Tokio, Tower, and friends
#   actix-battery    0.5.0  Actix-web ecosystem bundle
#   rocket-battery   0.3.0  Rocket framework with common extensions
```

## Skills for AI

Skills are markdown files with structured guidance:

```markdown
<!-- skills/async-patterns.md -->
# Async Patterns in web-battery

## Spawning Background Tasks

When you need to run work in the background:

\```rust
use tokio::spawn;

let handle = spawn(async {
    // background work
});
\```

## Graceful Shutdown

The recommended pattern for graceful shutdown:

\```rust
use tokio::signal;

async fn shutdown_signal() {
    signal::ctrl_c().await.expect("failed to listen for ctrl-c");
}
\```

## Common Pitfalls

- Don't hold locks across await points
- Use `tokio::select!` for racing futures
- Prefer channels over shared state
```

These can be consumed by AI agents working on projects that use the battery pack.

## Future Ideas

- **`cargo battery doctor`** — diagnose common issues in a battery-pack-generated project
- **Compatibility matrix** — track which versions of crates work together
- **Telemetry** (opt-in) — understand which templates/features are popular
- **IDE integration** — VS Code extension that surfaces skills contextually
- **Cross-battery dependencies** — battery packs that depend on other battery packs

## Open Questions

1. Should templates live in the battery pack repo or external repos?
2. How to handle breaking changes in underlying crates?
3. Governance: who decides what's "curated"?
4. Should there be an official "blessed" set of battery packs?

## Summary

Battery Pack provides:

| Component | Purpose |
|-----------|---------|
| `battery-pack` crate | Framework for building battery packs |
| Crate re-exports | Single dependency, access to curated crates |
| Runnable binary | CLI for each battery pack |
| cargo-generate integration | Templates for getting started |
| Skills | AI-readable guidance |
| Upgrade support | Lifecycle management |

The goal: make it easy to start a Rust project with confidence that you're using crates that work well together, with guidance on how to use them effectively. Users interact directly with the underlying crates—the battery pack is curation, not abstraction.
