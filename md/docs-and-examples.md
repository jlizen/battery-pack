# Documentation and Examples

A battery pack's documentation shows up in two places: on crates.io (from README.md)
and on docs.rs (from the auto-generated lib docs). The doc generation system
lets you write prose naturally while getting an auto-generated crate catalog for free.

## How it works

The documentation pipeline has three pieces:

1. **README.md** — your hand-written prose, displayed on crates.io
2. **docs.handlebars.md** — a template that controls what appears on docs.rs
3. **build.rs** — renders the template into `docs.md` at build time

The generated `docs.md` is included by `lib.rs`:

```rust
#![doc = include_str!(concat!(env!("OUT_DIR"), "/docs.md"))]
```

## Writing your README

Write a normal README.md. It should explain what your battery pack provides,
when to use which crates, and any guidance that helps users get started.

For example, `error-battery-pack`'s README might say:

```markdown
# error-battery-pack

Error handling done well — anyhow for apps, thiserror for libraries.

## When to use what

- **anyhow** — Use in application code where you want easy error
  propagation with context. Great for `main()`, CLI handlers,
  and integration tests.

- **thiserror** — Use in library code where you want to define
  structured error types that callers can match on.
```

## The handlebars template

The `docs.handlebars.md` file is a [Handlebars](https://handlebarsjs.com/) template
that controls what goes into the docs.rs documentation. The default template
shipped by `cargo bp new` looks like:

```handlebars
{{readme}}

{{crate-table}}
```

- `{{readme}}` — includes the contents of your README.md
- `{{crate-table}}` — renders an auto-generated table of all curated crates

### The crate table

The `{{crate-table}}` helper generates a table listing each crate in
the battery pack with its version, description, and a link to crates.io.
The descriptions are pulled automatically from crate metadata
(via `cargo metadata`), so you don't have to maintain them by hand.

When the `battery-pack` crate updates, the table's formatting improves
automatically for all battery packs that use `{{crate-table}}`.

### Custom templates

If you want full control, replace `{{crate-table}}` with your own
Handlebars markup. The same metadata is available in structured form:

```handlebars
{{readme}}

## Curated Crates

| Crate | Version | Description |
|-------|---------|-------------|
{{#each crates}}
| [{{name}}](https://crates.io/crates/{{name}}) | {{version}} | {{description}} |
{{/each}}

{{#if features}}
## Features

{{#each features}}
### `{{name}}`
{{#each crates}}
- {{this}}
{{/each}}
{{/each}}
{{/if}}
```

The available template variables include:

- `crates` — array of `{ name, version, description, features, dep_kind }`
- `features` — array of `{ name, crates }` from `[features]`
- `readme` — the contents of README.md
- `package` — `{ name, version, description, repository }`

## Writing examples

Examples are standard Cargo examples in the `examples/` directory.
They serve two purposes: showing users how to use the curated crates together,
and appearing in the battery pack's listing (in the TUI and `cargo bp show`).

Good examples:
- Are self-contained and runnable
- Show the crates working together (not just one crate in isolation)
- Cover common use cases for the battery pack's domain

```rust
// examples/basic.rs
use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("config error: {0}")]
struct ConfigError(String);

fn main() -> Result<()> {
    let path = "config.toml";
    let _content = std::fs::read_to_string(path)
        .context("reading config file")?;
    Ok(())
}
```

Examples listed in the TUI link to the source on GitHub
(when a repository URL is provided in Cargo.toml).
