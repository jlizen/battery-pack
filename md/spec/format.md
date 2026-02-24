# Battery Pack Format

This section specifies the structure of a battery pack crate.

## Crate structure

r[format.crate.name]
A battery pack crate's name MUST end in `-battery-pack`
(e.g., `error-battery-pack`, `cli-battery-pack`).

r[format.crate.keyword]
A battery pack crate MUST include `battery-pack` in its `keywords`
so that `cargo bp list` can discover it via the crates.io API.

r[format.crate.lib]
A battery pack crate's `lib.rs` SHOULD contain only a doc include directive.
There is no functional code in a battery pack.

r[format.crate.no-code]
A battery pack crate MUST NOT contain functional Rust code
(beyond the doc include and build.rs for doc generation).
It exists purely as a metadata and documentation vehicle.

## Dependencies as curation

r[format.deps.source-of-truth]
The battery pack's dependency sections (`[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]`) are the source of truth
for which crates the battery pack curates and their recommended
versions and features.

r[format.deps.kind-mapping]
The dependency section a crate appears in determines the default
dependency kind for users. A `[dependencies]` entry defaults to a
regular dependency, `[dev-dependencies]` to a dev-dependency, and
`[build-dependencies]` to a build-dependency.

r[format.deps.version-features]
Each dependency entry specifies the recommended version and Cargo features.
These are used by `cargo bp` when adding the crate to a user's project.

## Features

r[format.features.grouping]
Cargo `[features]` in the battery pack define named groups of crates.
Each feature lists the crate names it includes.

r[format.features.default]
The `default` feature determines which crates are installed when
a user runs `cargo bp add <pack>` without additional flags.

r[format.features.no-default]
If no `default` feature is defined, all non-optional crates are
considered part of the default set.

r[format.features.optional]
Crates marked `optional = true` in the dependency section are not
part of the default installation. They are available through named
features or individual selection.

r[format.features.additive]
Features are additive. Enabling a feature adds its crates on top of
whatever is already enabled. Features never remove crates.

r[format.features.augment]
A feature MAY augment the Cargo features of a crate that is already
included via another feature or the default set. When augmenting,
the specified Cargo features are unioned with the existing set.

## Hidden dependencies

r[format.hidden.metadata]
The `[package.metadata.battery-pack]` section MAY contain a `hidden`
key with a list of dependency names to hide from users.

r[format.hidden.effect]
Hidden dependencies do not appear in the TUI, `cargo bp show`,
or the auto-generated crate table. They cannot be installed by users
through `cargo bp`.

r[format.hidden.glob]
Entries in the `hidden` list MAY use glob patterns.
For example, `"serde*"` hides `serde`, `serde_json`, `serde_derive`, etc.

r[format.hidden.wildcard]
The value `"*"` hides all dependencies. This is useful for battery packs
that provide only templates and examples.

## Templates

r[format.templates.directory]
Templates are stored in subdirectories under `templates/` in the
battery pack crate.

r[format.templates.metadata]
Templates MUST be registered in `[package.metadata.battery.templates]`
with a `path` and `description`:

```toml
[package.metadata.battery.templates]
default = { path = "templates/default", description = "A basic starting point" }
```

r[format.templates.cargo-generate]
Templates use the [cargo-generate](https://github.com/cargo-generate/cargo-generate)
format. Each template directory MUST contain a `cargo-generate.toml`.

r[format.templates.selection]
If a battery pack has multiple templates, `cargo bp new` MUST prompt
the user to select one (unless `--template` is specified).

## Examples

r[format.examples.standard]
Examples are standard Cargo examples in the `examples/` directory.
They follow normal Cargo conventions and are runnable with `cargo run --example`.

r[format.examples.browsable]
Examples MUST be listed in `cargo bp show` output and in the TUI's
detail view for the battery pack.
