# Manifest Manipulation

This section specifies how `cargo bp` reads and modifies Cargo.toml files.

## Battery pack registration

r[manifest.register.location]
Battery pack registrations are stored in a `[*.metadata.battery-pack]`
table, where `*` is either `package` or `workspace`.

r[manifest.register.format]
Each registration is a key-value pair where the key is the battery pack
crate name and the value is the version string:

```toml
[package.metadata.battery-pack]
error-battery-pack = "0.4.0"
cli-battery-pack = "0.3.0"
```

r[manifest.register.workspace-default]
When a workspace root exists, battery pack registrations MUST default
to `[workspace.metadata.battery-pack]` in the workspace root Cargo.toml.

r[manifest.register.package-level]
The user MAY choose to register a battery pack at the package level
using `[package.metadata.battery-pack]` in the crate's own Cargo.toml.
This is for per-crate battery packs in a workspace.

r[manifest.register.both-levels]
`cargo bp` MUST support reading registrations from both workspace
and package metadata. When both exist, package-level registrations
take precedence for that crate.

## Active features

r[manifest.features.storage]
The active features for a battery pack MUST be stored alongside
the registration. The format is:

```toml
[package.metadata.battery-pack]
cli-battery-pack = { version = "0.3.0", features = ["default", "indicators"] }
```

r[manifest.features.default-implicit]
If no `features` key is present, the `default` feature is implicitly active.

r[manifest.features.short-form]
When only the default feature is active, the registration MAY use
the short form (just a version string) instead of a table.

## Dependency management

r[manifest.deps.add]
When adding a crate, `cargo bp` MUST add it to the correct dependency
section (`[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]`)
based on the battery pack's Cargo.toml, unless overridden by the user.

r[manifest.deps.version-features]
Each dependency entry MUST include the version and Cargo features
as specified by the battery pack.

r[manifest.deps.workspace]
In a workspace, `cargo bp` MUST add crate entries to
`[workspace.dependencies]` in the workspace root and reference
them as `crate = { workspace = true }` in the crate's dependency section.

r[manifest.deps.no-workspace]
In a non-workspace project, `cargo bp` MUST add crate entries
directly to the crate's dependency section with full version and features.

r[manifest.deps.existing]
If a dependency already exists in the user's Cargo.toml, `cargo bp`
MUST NOT overwrite user customizations (additional features, version overrides).
It MUST only add missing features and warn about version mismatches.

r[manifest.deps.remove]
When a user disables a crate via the TUI, `cargo bp` MUST remove
it from the appropriate dependency section. If using workspace
dependencies, the `workspace.dependencies` entry SHOULD be preserved
(other crates in the workspace may use it).

## Sync behavior

r[manifest.sync.version-bump]
During sync, if a dependency's version is older than what the
battery pack recommends, `cargo bp` MUST update it to the
recommended version.

r[manifest.sync.feature-add]
During sync, if a dependency is missing Cargo features that the
battery pack specifies, `cargo bp` MUST add them.

r[manifest.sync.no-downgrade]
`cargo bp` MUST NOT downgrade a dependency version during sync.
If the user has a newer version, it is left unchanged.

r[manifest.sync.no-feature-remove]
`cargo bp` MUST NOT remove Cargo features from a dependency
during sync. User-added features are preserved.

## TOML formatting

r[manifest.toml.preserve]
`cargo bp` MUST preserve existing TOML formatting, comments,
and ordering when modifying Cargo.toml files.

r[manifest.toml.style]
New entries added by `cargo bp` SHOULD follow the existing
formatting style of the file (inline tables vs. multi-line, etc.).
