# Publishing

Battery packs are published to crates.io like any other Rust crate.
A few things to keep in mind to make yours discoverable and useful.

## Keywords

Include `battery-pack` as a keyword in your Cargo.toml so `cargo bp list`
can find it:

```toml
[package]
name = "error-battery-pack"
keywords = ["battery-pack", "error-handling", "anyhow", "thiserror"]
```

The `battery-pack` keyword is what `cargo bp` uses to discover packs on crates.io.
Add other keywords relevant to your domain.

## Naming

Battery packs conventionally end in `-battery-pack`:

- `error-battery-pack`
- `cli-battery-pack`
- `async-battery-pack`
- `web-battery-pack`

The `cargo bp` CLI resolves short names automatically — `cargo bp add cli`
looks up `cli-battery-pack`. If you name your crate `my-cool-battery-pack`,
users can just type `cargo bp add my-cool`.

## Versioning

Follow semver, but think about what constitutes a breaking change
for a battery pack:

- **Patch** — updating a crate's patch version, fixing docs or examples
- **Minor** — adding new crates, adding new features (groups), adding new templates
- **Major** — removing crates, bumping a crate's major version, removing features

When you bump a curated crate's version, users will see a warning in
`cargo bp status` if their installed version is older. They can update
with `cargo bp sync`.

## Pre-publish checklist

1. **README.md** describes the battery pack clearly
2. **Examples** are runnable (`cargo test --examples`)
3. **Templates** work (`cargo bp new your-pack` from a temp directory)
4. **Keywords** include `battery-pack`
5. **License** files are present (MIT and/or Apache-2.0 are conventional)
6. **Repository** URL is set (for linking to examples and templates in the TUI)

## Publishing

```bash
cargo publish
```

After publishing, your battery pack will appear in `cargo bp list`
within a few minutes (once the crates.io index updates).
