# Using Battery Packs

## The TUI

The primary way to interact with battery packs is the terminal UI.
Run `cargo bp` with no arguments:

```bash
cargo bp
```

The TUI is context-dependent. If you're inside a Rust project, you'll see:

- **Your installed battery packs** — toggle individual crates on and off,
  change dependency kinds, enable features
- **Browse** — search and add new battery packs from crates.io
- **New project** — create a project from a battery pack template

If you're not in a Rust project, the installed-packs section is
greyed out, but you can still browse and create new projects.

## Non-interactive mode

Pass `--non-interactive` (or `-N`) to suppress TUI and prompts:

```bash
cargo bp -N list                        # plain text output
cargo bp -N show cli                    # plain text output
cargo bp -N new cli --name my-app       # no prompts
cargo bp -N rm cli                      # skip "remove deps?" prompt
```

Set `CARGO_BP_NON_INTERACTIVE=true` to activate for an entire session:

```bash
export CARGO_BP_NON_INTERACTIVE=true
cargo bp list              # plain text, no TUI
cargo bp new cli -n my-app # no prompts
```

For `new`, use `--name` and `-d` to provide values that would otherwise be prompted:

```bash
cargo bp new cli --name my-app -d description="My CLI tool"
```

## Browsing available packs

### From the TUI

The Browse tab in the TUI lets you search crates.io for battery packs.
Select one to see its contents — which crates it includes, what features
it offers, and what templates are available.

### From the command line

```bash
cargo bp list              # list all battery packs
cargo bp list cli          # filter by name
cargo bp show cli          # detailed view of cli-battery-pack
```

## Creating a new project from a template

Some battery packs ship with project templates that scaffold a new Rust project with the recommended crates and project structure already wired up.

```bash
cargo bp new cli
```

You'll be prompted for a project name and directory. The result is a ready-to-go Rust project with the battery pack's recommended crates already in your `Cargo.toml`.

### Choosing a template

If a battery pack offers multiple templates, you can pick one:

```bash
cargo bp new cli --template simple
cargo bp new cli --template subcmds
```

### Previewing output

To see what a template will generate without writing any files:

```bash
cargo bp show cli -t default
```

In a terminal, this opens an interactive scrollable preview. For plain text output (e.g. in CI or piped to another command):

```bash
cargo bp show cli -t default --non-interactive
```

For `new`, use `--name` and `-d` to provide values that would otherwise be prompted:

```bash
cargo bp new cli --name my-app -d description="My CLI tool"
```

You can also create new projects from the TUI's "New project" tab.

## Merging a template into an existing project

Some battery packs include small, single-purpose templates designed to augment an existing project rather than scaffold a new one. For example, `ci-battery-pack` has templates for spellcheck, fuzzing, benchmarks, and more. You can merge these into your project with `cargo bp add -t`:

```bash
cargo bp add ci -t spellcheck
cargo bp add ci -t fuzzing -d ci_platform=github
cargo bp add ci -t trusted-publishing
```

The merge is format-aware:

| File type | New file | Existing file |
|-----------|----------|---------------|
| `Cargo.toml` | Write | Merge: deps upgraded if behind, features unioned |
| Other `.toml` files | Write | Merge: sections/keys inserted if absent, preserved if present |
| `.yml` / `.yaml` files | Write | Merge: top-level keys added, existing keys left alone |
| Everything else | Write | You're prompted to skip, overwrite, or view a diff |

For TOML and YAML conflicts, you can accept the merge, skip it, or open the result in `$EDITOR` to modify before writing. For other files, you can skip, overwrite, or view a diff. Both prompt types include batch options ("skip all", "accept all" / "overwrite all") so you don't get prompted repeatedly. Each option has a single-key shortcut (shown in brackets) so you don't need to arrow through a menu.

Flags that change the behavior:

- **`--overwrite`**: overwrites other files without prompting. TOML and YAML files are always merged, never overwritten.
- **`-N` (non-interactive)**: skips other-file conflicts unless `--overwrite` is also passed. TOML and YAML merges apply automatically.

If your working tree has uncommitted changes, `cargo bp` warns before proceeding. In non-interactive mode, it refuses unless `--overwrite` is passed. Commit or stash first for a clean undo path.

The project name for template variables is inferred from your `Cargo.toml` `[package].name`.

Some templates print follow-up instructions after the merge (e.g., "add `mod errors;` to your lib.rs") to guide you through manual steps.

In the TUI, you can merge templates by selecting one in the detail view and pressing `u` (Use in project).

## Adding a battery pack

### Basic add

```bash
cargo bp add cli
```

This adds the battery pack's **default** crates to your project.
Which crates are "default" is determined by the battery pack author
(see [Features](#features) below).

### Adding with features

```bash
cargo bp add cli -F indicators
```

This adds the default crates plus the crates from the `indicators` feature.
You can also write `--features indicators`, or enable multiple features
with `-F indicators,fancy` or repeated `-F indicators -F fancy`.

### Adding with no defaults

```bash
cargo bp add cli --no-default-features -F indicators
```

This adds only the `indicators` feature's crates, skipping the defaults.

### Adding everything

```bash
cargo bp add cli --all-features
```

This adds every crate the battery pack offers, regardless of defaults or features.

### Adding specific crates

```bash
cargo bp add cli clap indicatif
```

This adds just the named crates from the battery pack.

## Features

Battery packs use Cargo's `[features]` to group related crates.
For example, `cli-battery-pack` might define:

```toml
[features]
default = ["clap", "dialoguer"]
indicators = ["indicatif", "console"]
fancy = ["clap", "indicatif", "console"]
```

- **default** — the crates you get with a plain `cargo bp add cli`
- **indicators** — progress bars and console styling
- **fancy** — argument parsing with color support, plus indicators

Features are additive. Enabling `indicators` on top of `default` gives you
all four crates. A feature can also augment the Cargo features of a crate
that's already included (e.g., adding the `color` feature to `clap`).

In the TUI, features appear as toggleable groups alongside individual crate toggles.

## Dependency kinds

By default, each crate is added with the same dependency kind it has in the
battery pack's Cargo.toml:

- A crate listed in the battery pack's `[dev-dependencies]` becomes a
  `[dev-dependencies]` entry in your project
- A crate in `[dependencies]` becomes a regular dependency
- A crate in `[build-dependencies]` becomes a build dependency

You can override this in the TUI — for instance, promoting a dev-dependency
to a regular dependency, or vice versa.

## Keeping in sync

### Checking status

```bash
cargo bp status
```

This shows your installed battery packs and highlights any mismatches.
If a battery pack recommends `clap 4.5` but you have `clap 4.3`, you'll
see a warning. Having a *newer* version than recommended is fine.

### Syncing

```bash
cargo bp sync
```

This updates your dependencies to match the installed battery packs:

- Bumps versions that are older than what the battery pack recommends
- Adds features the battery pack has added since your last sync
- Adds new crates if they've been added to your active features

Sync is non-destructive — it only adds and upgrades, never removes.

## Workspaces

When your crate is part of a Cargo workspace, `cargo bp` is workspace-aware:

- Battery pack registrations go in `[workspace.metadata.battery-pack]`
  by default (you can toggle this in the TUI to use per-crate metadata instead)
- Dependencies are added to `[workspace.dependencies]` and referenced
  as `crate = { workspace = true }` in the crate's `[dependencies]`

This keeps versions centralized and consistent across workspace members.

For per-crate battery packs (where only one workspace member uses a pack),
you can store the registration and dependencies at the crate level instead.

## Local sources

You can point `cargo bp` at a local workspace containing battery packs
instead of (or in addition to) crates.io. This is useful for:

- **Testing** — validate your battery pack before publishing
- **Organizations** — maintain internal battery packs in a monorepo
- **Development** — iterate on a battery pack alongside the project using it

```bash
cargo bp --source ../my-battery-packs add cli
cargo bp --source ../my-battery-packs
```

The `--source` flag takes a path to a Cargo workspace. `cargo bp`
discovers all `*-battery-pack` crates within it automatically.
Local sources take precedence over crates.io, so if both have
`cli-battery-pack`, the local one wins.

You can combine multiple sources:

```bash
cargo bp --source ../team-packs --source ../my-packs list
```

For a single battery pack directory (not a workspace), use `--path`:

```bash
cargo bp add my-pack --path ../my-battery-pack
```

## Multiple battery packs

A project can use multiple battery packs:

```toml
[package.metadata.battery-pack]
error-battery-pack = "0.4.0"
cli-battery-pack = "0.3.0"
async-battery-pack = "0.2.0"

[dependencies]
anyhow = "1"
thiserror = "2"
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

Each battery pack tracks its own metadata. If two battery packs
recommend the same crate with different features, the features are
merged (unioned) — this is always safe.
