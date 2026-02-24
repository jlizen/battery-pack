# Plan: Rewrite the mdbook from scratch

## Context

The existing mdbook (`md/`) is outdated — it describes the facade approach (`use cli::clap::Parser`) which no longer exists, contains two internal design docs (option2/option3) that were working documents, and has placeholder pages. The vision for battery packs has evolved significantly:

- Battery packs are **documentation packages** — no real code, just deps + metadata + templates + examples
- No build-time validation — no validate(), no build.rs on the user side
- `cargo bp` (no args) is the primary TUI interface
- Battery pack registration lives in workspace/package metadata, not build-dependencies
- Cargo `[features]` define crate groupings (replacing custom "sets" metadata)
- `optional = true` means "not in default" — battery pack dep sections mirror user dep kinds
- Hidden deps (`hidden = ["*"]`, globs) for internal tooling
- Auto-generated docs via handlebars build.rs
- Tracey for linking spec requirements to code

The book needs to be rewritten as three main sections: a user's guide, an author's guide, and a specification with tracey-tagged requirements.

## Proposed structure

```
SUMMARY.md

# User's Guide
├── Introduction (intro.md)
│   - What is a battery pack?
│   - Philosophy: curation, not abstraction
│   - Quick taste: adding a battery pack in 30 seconds
│   - How it works at a high level
│
├── Getting Started (getting-started.md)
│   - Installing the CLI
│   - Creating a project from a template (cargo bp new)
│   - Adding a battery pack to an existing project (cargo bp add)
│   - What changed in your Cargo.toml
│
├── Using Battery Packs (using.md)
│   - The TUI (cargo bp with no args)
│   - Browsing and searching (cargo bp list, cargo bp show)
│   - Adding packs (cargo bp add)
│   - Features: enabling groups of crates
│   - Choosing dependency kind (dev/build/runtime)
│   - Keeping in sync (cargo bp status, cargo bp sync)
│   - Workspaces
│   - Multiple battery packs in one project

# Author's Guide
├── Creating a Battery Pack (creating.md)
│   - Scaffolding with cargo bp new
│   - The anatomy of a battery pack
│   - Defining crates via dependencies
│   - Dependency kinds (dev vs regular vs build)
│   - Features for grouping crates
│   - The default feature and optional = true
│   - Hidden dependencies
│
├── Documentation and Examples (docs-and-examples.md)
│   - Writing your README.md
│   - The handlebars template (docs.handlebars.md)
│   - Built-in helpers: {{crate-table}} etc.
│   - Custom templates with structured metadata
│   - The lib.rs pattern (include generated docs)
│   - Writing examples
│   - Templates for cargo bp new
│
├── Publishing (publishing.md)
│   - Publishing to crates.io
│   - Keywords and discoverability
│   - Versioning strategy

# Specification
├── Battery Pack Format (spec/format.md)
│   - r[format.*] requirements for crate structure
│   - Dependency sections and their meaning
│   - Features and grouping
│   - Hidden dependencies
│   - Metadata sections
│   - Templates directory layout
│
├── CLI Behavior (spec/cli.md)
│   - r[cli.*] requirements for each subcommand
│   - cargo bp (TUI)
│   - cargo bp add
│   - cargo bp new
│   - cargo bp status
│   - cargo bp sync
│   - cargo bp list
│   - cargo bp show
│
├── TUI Behavior (spec/tui.md)
│   - r[tui.*] requirements for the interactive interface
│   - Main menu and context detection
│   - Installed packs view
│   - Browse view
│   - Dependency toggling and kind selection
│   - Template / new project flow
│
├── Manifest Manipulation (spec/manifest.md)
│   - r[manifest.*] requirements
│   - Workspace vs package metadata
│   - Where battery pack registrations are stored
│   - How dependencies are added/synced
│   - Workspace dependency management
│
├── Documentation Generation (spec/docgen.md)
│   - r[docgen.*] requirements
│   - build.rs flow
│   - Handlebars template processing
│   - Built-in helpers
│   - Metadata extraction via cargo metadata
```

## Approach

1. Delete all existing `.md` files in `md/` (outdated or placeholders; preserved in git history)
2. Create `md/spec/` subdirectory for specification pages
3. Write SUMMARY.md with the structure above
4. Write each chapter in order: user's guide first, then author's guide, then spec
5. Spec pages use `r[requirement.id]` tracey syntax for each requirement
6. Set up tracey config (`.config/tracey/config.styx`) pointing at `md/spec/` for specs and `src/` for implementations

## Writing guidelines

- Niko's voice: direct, conversational, technically precise
- Audience knows Rust — don't explain Cargo basics
- Use `error-battery-pack` as the simple running example (anyhow + thiserror)
- Use `cli-battery-pack` for feature/grouping examples (has indicators feature)
- Show real Cargo.toml snippets reflecting the NEW vision (metadata-based registration, no build-deps)
- Spec requirements: granular, independently testable, additive (each stands on its own)
- Keep pages scannable — headers, short paragraphs, code blocks

## Key examples to use throughout

**Battery pack Cargo.toml (cli-battery-pack):**
```toml
[package]
name = "cli-battery-pack"
version = "0.3.0"

[dev-dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = { version = "0.17", optional = true }
console = { version = "0.15", optional = true }

[features]
default = ["clap", "dialoguer"]
indicators = ["indicatif", "console"]
```

**User's Cargo.toml after `cargo bp add cli`:**
```toml
[package.metadata.battery-pack]
cli-battery-pack = "0.3.0"

[dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
```

**User's Cargo.toml after enabling indicators:**
```toml
[package.metadata.battery-pack]
cli-battery-pack = "0.3.0"

[dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"
```

## Files to create/modify

- `md/SUMMARY.md` — new table of contents
- `md/intro.md` — complete rewrite
- `md/getting-started.md` — new
- `md/using.md` — complete rewrite
- `md/creating.md` — new (replaces authoring.md)
- `md/docs-and-examples.md` — new
- `md/publishing.md` — new
- `md/spec/format.md` — new
- `md/spec/cli.md` — new
- `md/spec/tui.md` — new
- `md/spec/manifest.md` — new
- `md/spec/docgen.md` — new
- `.config/tracey/config.styx` — new (tracey configuration)
- Delete: `md/option2.md`, `md/option3.md`, `md/authoring.md`

## Order of operations

1. Delete old files, create directory structure
2. Write SUMMARY.md
3. Write intro.md
4. Write getting-started.md
5. Write using.md
6. Write creating.md
7. Write docs-and-examples.md
8. Write publishing.md
9. Write spec/format.md (with r[] tags)
10. Write spec/cli.md (with r[] tags)
11. Write spec/tui.md (with r[] tags)
12. Write spec/manifest.md (with r[] tags)
13. Write spec/docgen.md (with r[] tags)
14. Set up tracey config
15. Verify with `mdbook build`

## Verification

- `mdbook build` succeeds with no broken links
- All code examples reflect the new vision (no facade, no build-deps, no validate)
- Spec requirements are granular and independently testable
- Tracey config points at the right files
- Read-through for consistency and accuracy
