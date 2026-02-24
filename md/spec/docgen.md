# Documentation Generation

This section specifies how battery pack documentation is automatically
generated for display on docs.rs.

## Build-time generation

r[docgen.build.trigger]
The battery pack's `build.rs` MUST generate a `docs.md` file
in `OUT_DIR` during the build process.

r[docgen.build.template]
The `build.rs` MUST read a Handlebars template file
(`docs.handlebars.md`) from the crate root and render it
with structured metadata.

r[docgen.build.lib-include]
The battery pack's `lib.rs` MUST include the generated documentation
via `#![doc = include_str!(concat!(env!("OUT_DIR"), "/docs.md"))]`.

## Template processing

r[docgen.template.handlebars]
The template format MUST be [Handlebars](https://handlebarsjs.com/).
The template file MUST be named `docs.handlebars.md`.

r[docgen.template.default]
The default template provided by `cargo bp new` MUST include
the README and a crate table:

```handlebars
{{readme}}

{{crate-table}}
```

r[docgen.template.custom]
Battery pack authors MAY customize the template to control
the documentation layout. The same structured metadata available
to built-in helpers MUST also be available as template variables
for custom markup.

## Built-in helpers

r[docgen.helper.readme]
The `{{readme}}` helper MUST expand to the contents of the
battery pack's `README.md`.

r[docgen.helper.crate-table]
The `{{crate-table}}` helper MUST render a table of all
non-hidden curated crates, including each crate's name
(linked to crates.io), version, and description.

r[docgen.helper.crate-table-metadata]
Crate descriptions in `{{crate-table}}` MUST be sourced from
crate metadata (via `cargo metadata`), not manually maintained.

r[docgen.helper.crate-table-update]
The `{{crate-table}}` implementation lives in the `bphelper` crate.
Updating `bphelper` MUST automatically update the table rendering
for all battery packs that use `{{crate-table}}`.

## Template variables

r[docgen.vars.crates]
The template context MUST include a `crates` array. Each entry
MUST have: `name`, `version`, `description`, `features` (Cargo features),
and `dep_kind` (dependencies, dev-dependencies, or build-dependencies).

r[docgen.vars.features]
The template context MUST include a `features` array. Each entry
MUST have: `name` and `crates` (list of crate names in that feature).

r[docgen.vars.readme]
The template context MUST include a `readme` string containing
the contents of the battery pack's `README.md`.

r[docgen.vars.package]
The template context MUST include a `package` object with:
`name`, `version`, `description`, and `repository`.

## Hidden crates

r[docgen.hidden.excluded]
Crates listed in the battery pack's `hidden` configuration
MUST NOT appear in the `crates` template variable or in the
output of `{{crate-table}}`.
