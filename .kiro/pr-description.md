# Use dotted key syntax for `bp-managed` dependencies

Follows up on #49.

### Summary

Switches all `bp-managed` usage from inline table syntax to dotted key syntax:

```toml
# Before
anyhow = { bp-managed = true }

# After
anyhow.bp-managed = true
```

Per feedback from @nikomat: this is shorter and more idiomatic TOML.

Both syntaxes are equivalent in TOML and the resolution code already
handled both (the `is_bp_managed_item` function matches on both
`InlineTable` and `Table` variants). This PR updates all templates,
fixtures, tests, and docs to use the dotted key form as the canonical
style.

Changes:
- Templates: `cli-battery-pack` (simple, subcmds), `battery-pack`
  (with_template default), `managed-battery-pack` fixture
- Tests: primary test inputs switched to dotted keys, added explicit
  backward compat test for inline table syntax
- Docs: `creating.md` and `spec/manifest.md` examples updated

### Testing

- All 308 tests pass (including template validation for cli-battery-pack
  and battery-pack, which generate projects and run `cargo check` +
  `cargo test` on them)
- Added `resolve_bp_managed_inline_table_syntax` test to verify the
  old inline table form still resolves correctly
- `cargo fmt --check` and `cargo clippy` clean
