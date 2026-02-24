# CLI Behavior

This section specifies the behavior of each `cargo bp` subcommand.

## Local sources

r[cli.source.flag]
`cargo bp --source <path>` MUST add a local workspace as a
battery pack source. The `<path>` MUST point to a directory
containing a `Cargo.toml` with `[workspace]`.

r[cli.source.discover]
When a local source is specified, `cargo bp` MUST scan the
workspace members for crates whose names end in `-battery-pack`
and make them available as battery packs.

r[cli.source.precedence]
Local sources MUST take precedence over crates.io. If a battery
pack exists in both a local source and crates.io, the local
version MUST be used.

r[cli.source.multiple]
The `--source` flag MAY be specified multiple times to add
multiple local workspaces.

r[cli.source.subcommands]
The `--source` flag MUST be accepted by all subcommands that
resolve battery packs: `add`, `new`, `show`, `list`, `status`,
and `sync`, as well as the bare `cargo bp` TUI.

r[cli.source.scope]
The `--source` flag is a per-invocation option that adds
additional directories to the set of places `cargo bp` searches
for battery packs. It does not persist across invocations.

## Path flag

r[cli.path.flag]
`cargo bp --path <path>` MUST read a battery pack from the
given directory. Unlike `--source`, which adds a searchable
workspace, `--path` identifies a single battery pack directory
directly.

r[cli.path.subcommands]
The `--path` flag MUST be accepted by all subcommands that
operate on a specific battery pack: `add`, `new`, `show`,
`validate`, `status`, and `sync`.

r[cli.path.no-resolve]
When `--path` is provided, name resolution is not needed.
The battery pack is read directly from the given directory.

## Name resolution

r[cli.name.resolve]
When a battery pack name is given without the `-battery-pack` suffix,
the CLI MUST resolve it by appending `-battery-pack`.
For example, `cli` resolves to `cli-battery-pack`.

r[cli.name.exact]
If the user provides a full crate name ending in `-battery-pack`,
it MUST be used as-is without further modification.

## `cargo bp` (no arguments)

r[cli.bare.tui]
Running `cargo bp` with no subcommand MUST launch the interactive TUI.

r[cli.bare.help]
Running `cargo bp --help` MUST print CLI help text and exit
(not launch the TUI).

## `cargo bp add`

r[cli.add.register]
`cargo bp add <pack>` MUST register the battery pack in the project's
metadata and add the default crates to the appropriate dependency sections.

r[cli.add.default-crates]
When no `-F`/`--features`, `--no-default-features`, or `--all-features`
flags are given, `cargo bp add <pack>` MUST add the crates from the
battery pack's `default` feature (or all non-optional crates if no
`default` feature exists).

r[cli.add.features]
`cargo bp add <pack> -F <name>` (or `--features <name>`) MUST add
the default crates plus all crates from the named feature.

r[cli.add.features-multiple]
Multiple features MAY be specified as a comma-separated list
(`-F indicators,fancy`) or by repeating the flag (`-F indicators -F fancy`).

r[cli.add.no-default-features]
`cargo bp add <pack> --no-default-features` MUST skip the default crates.
Combined with `-F`, it adds only the named feature's crates.

r[cli.add.all-features]
`cargo bp add <pack> --all-features` MUST add every crate the battery pack
offers, regardless of features or optional status.

r[cli.add.specific-crates]
`cargo bp add <pack> <crate> [<crate>...]` MUST add only the
named crates from the battery pack, ignoring defaults and features.

r[cli.add.dep-kind]
Each crate MUST be added with the dependency kind matching its
section in the battery pack's Cargo.toml (regular, dev, or build),
unless the user overrides it.

r[cli.add.target]
`cargo bp add <pack> --target <level>` controls where the battery
pack registration is stored. The `<level>` MUST be one of:
- `workspace` — register in `[workspace.metadata.battery-pack]`
- `package` — register in `[package.metadata.battery-pack]`
- `default` — use workspace if a workspace root exists, otherwise package

If `--target` is not specified, the default behavior MUST be used.

r[cli.add.unknown-crate]
When specific crates are named (`cargo bp add <pack> <crate>...`)
and a named crate does not exist in the battery pack, `cargo bp`
MUST report an error for that crate. Other valid crates in the
same command MUST still be processed.

r[cli.add.idempotent]
Adding a battery pack that is already registered MUST NOT create
duplicate entries. If the battery pack is already present,
`cargo bp add` MUST update its version and sync any new crates.

## `cargo bp new`

r[cli.new.template]
`cargo bp new <pack>` MUST create a new project from the battery
pack's template using cargo-generate.

r[cli.new.name-flag]
`cargo bp new <pack> --name <name>` MUST pass the project name
to cargo-generate, skipping the name prompt.

r[cli.new.name-prompt]
If `--name` is not provided, the CLI MUST prompt the user for
a project name (via cargo-generate).

r[cli.new.template-select]
If the battery pack has multiple templates and `--template` is not
provided, the CLI MUST prompt the user to select one.

r[cli.new.template-flag]
`cargo bp new <pack> --template <name>` MUST use the specified template
without prompting.

## `cargo bp status`

r[cli.status.list]
`cargo bp status` MUST list all installed battery packs with their
registered versions.

r[cli.status.version-warn]
For each installed battery pack, if any of the user's dependency
versions are older than what the battery pack recommends,
`cargo bp status` MUST display a warning.

r[cli.status.newer-ok]
If the user has a newer version of a crate than the battery pack
recommends, `cargo bp status` MUST NOT warn. Newer is acceptable.

r[cli.status.no-project]
If run outside a Rust project, `cargo bp status` MUST report
that no project was found.

## `cargo bp sync`

r[cli.sync.update-versions]
`cargo bp sync` MUST update dependency versions that are older
than what the installed battery packs recommend.

r[cli.sync.add-features]
`cargo bp sync` MUST add any Cargo features that the battery pack
specifies but are missing from the user's dependency entry.

r[cli.sync.add-crates]
`cargo bp sync` MUST add any crates that belong to the user's
active features but are missing from the user's dependencies.

r[cli.sync.non-destructive]
`cargo bp sync` MUST NOT remove crates or downgrade versions.
It only adds and upgrades.

## `cargo bp list`

r[cli.list.query]
`cargo bp list` MUST query crates.io for crates with the
`battery-pack` keyword.

r[cli.list.filter]
`cargo bp list <filter>` MUST filter results by name pattern.

r[cli.list.interactive]
If running in a TTY, `cargo bp list` SHOULD display results
in the interactive TUI.

r[cli.list.non-interactive]
`cargo bp list --non-interactive` MUST print results as plain text.

## `cargo bp validate`

r[cli.validate.purpose]
`cargo bp validate` MUST check whether a battery pack crate
conforms to the battery pack format specification (`format.*` rules).

r[cli.validate.default-path]
If `--path` is not provided, `cargo bp validate` MUST validate
the battery pack in the current directory.

r[cli.validate.checks]
`cargo bp validate` MUST check all applicable `format.*` rules,
including both data-level checks (from the parsed `Cargo.toml`)
and filesystem-level checks (on-disk structure).

r[cli.validate.severity]
Violations of MUST rules MUST be reported as errors.
Violations of SHOULD rules MUST be reported as warnings.

r[cli.validate.rule-id]
Each diagnostic MUST include the spec rule ID in its output
(e.g., `error[format.crate.name]: ...`).

r[cli.validate.clean]
When a battery pack passes all checks with no diagnostics,
`cargo bp validate` MUST print `<name> is valid` and exit
successfully.

r[cli.validate.warnings-only]
When a battery pack has warnings but no errors,
`cargo bp validate` MUST print `<name> is valid (<N> warning(s))`
and exit successfully.

r[cli.validate.errors]
When a battery pack has one or more errors, `cargo bp validate`
MUST exit with a non-zero status.

r[cli.validate.workspace-error]
If the target `Cargo.toml` is a workspace manifest (contains
`[workspace]` but no `[package]`), `cargo bp validate` MUST
report a clear error directing the user to run from a battery
pack crate directory or use `--path`.

r[cli.validate.no-package]
If the target `Cargo.toml` has no `[package]` section and is not
a workspace manifest, `cargo bp validate` MUST report a clear
error indicating the file is not a battery pack crate.

## `cargo bp show`

r[cli.show.details]
`cargo bp show <pack>` MUST display the battery pack's name, version,
description, curated crates, features, templates, and examples.

r[cli.show.hidden]
`cargo bp show` MUST NOT display hidden dependencies.

r[cli.show.interactive]
If running in a TTY, `cargo bp show` SHOULD display results
in the interactive TUI.

r[cli.show.non-interactive]
`cargo bp show --non-interactive` MUST print results as plain text.
