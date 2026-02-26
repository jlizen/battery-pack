# WIP: TUI Review Findings & Action Items

Previous task (TUI testing) is complete — 42 tests across 3 tiers, all passing.
Code review changes have been applied (see "Completed" section at end).
This document captures the remaining review findings and actionable items.

## Tracey Coverage Matrix

```
Rule                       impl       verify
tui.browse.add             -          6 tests
tui.browse.detail          -          2 tests
tui.browse.hidden          manifest   manifest
tui.browse.list            -          1 test
tui.browse.search          -          1 test
tui.installed.dep-kind     -          -            ** UNCOVERED **
tui.installed.features     -          1 test (partial)
tui.installed.hidden       manifest   manifest
tui.installed.list-crates  -          1 test
tui.installed.list-packs   -          1 test
tui.installed.show-state   -          5 tests
tui.installed.toggle-crate -          2 tests
tui.main.always-available  -          -            ** UNCOVERED **
tui.main.context-detection -          -            ** UNCOVERED **
tui.main.no-project        -          1 test (partial)
tui.main.sections          -          -            ** UNCOVERED **
tui.nav.cancel             -          1 test
tui.nav.exit               -          6 tests
tui.nav.keyboard           -          11 tests
tui.network.error          -          -            ** UNCOVERED **
tui.network.non-blocking   -          -            ** UNCOVERED **
tui.new.create             -          -            ** UNCOVERED **
tui.new.template-list      -          -            ** UNCOVERED **
```

8 rules have zero coverage. Most were already deferred (I/O-dependent).
The notable gap: `tui.installed.dep-kind` is not deferred and has no
implementation, no UI, and no test.

## Spec Comment Faithfulness

- `[impl format.hidden.effect]` (line 436): Cross-references `format.md`, not `tui.md`. Fine.
- `tui.installed.show-state`: Spec says dep_kind must be displayed. `CrateEntry::version_info()`
  shows version and features but never dep_kind. `CrateEntry` doesn't even store dep_kind.
  Tests tagged with this rule don't verify dep_kind display. Semantic gap.
- `tui.main.no-project`: Test verifies "No battery packs installed" (empty packs list).
  Spec requires detecting "no project found" and greying out. Different scenarios.
  Annotation says "(partial)" which is honest.

## Non-Additive Spec Rules

Four rule pairs where one rule subtracts from another:

1. **`tui.installed.features` subtracts from `tui.installed.toggle-crate`**:
   toggle-crate says toggling off "removes it"; features adds a condition
   ("crates that aren't required by another enabled feature").
   Fix: fold interaction into one rule.

2. **`tui.installed.hidden` subtracts from `tui.installed.list-crates`**:
   list-crates says "display its curated crates"; hidden says "except hidden ones".
   Fix: "display its curated crates (excluding hidden dependencies)".

3. **`tui.browse.hidden` subtracts from `tui.browse.detail`**: Same pattern.
   Fix: fold "excluding hidden" into browse.detail.

4. **`tui.nav.cancel` subtracts from `tui.nav.exit`**: Exit says "all pending
   changes MUST be applied"; cancel says "cancel without applying".
   Fix: make exit explicit about which exit paths apply changes.

## Duplicate Logic

### Wrapping navigation (4 copies)
`select_next`/`select_prev` with `(index + 1) % count` / `(index + count - 1) % count`
repeated in `DetailScreen`, `InstalledState`, `ExpandedPack`, and browse list handling.
Extract a `wrapping_nav(index: &mut usize, count: usize, forward: bool)` helper.

### `PendingAction::OpenCratesIo` duplicates `OpenUrl`
`OpenCratesIo` just builds a crates.io URL then does exactly what `OpenUrl` does.
Both `execute_action` arms are identical. Eliminate `OpenCratesIo` — callers
construct the URL and use `OpenUrl`.

### Browse list rendering duplicates list-screen pattern
`render_add_browse` builds `Vec<ListItem>` with the exact same formatting as
`render_list`: `short_name` + `version` + `description`, same colors, same
highlight style. Extract into a shared function.

### `FormField` dispatch repeated 5 times
Multiple `Action::Form*` arms repeat:
```rust
let field = match state.focused_field {
    FormField::Directory => &mut state.directory,
    FormField::ProjectName => &mut state.project_name,
};
```
Add `fn focused_field_mut(&mut self) -> &mut String` on `FormScreen`.

## Rust Idiom Issues

### `process_loading` uses `mem::replace` with a dummy sentinel
Constructs a meaningless `LoadingState` to satisfy `mem::replace`. Same pattern
in `take_add_screen_for_loading`. Consider `Option<Screen>` or `Screen::Empty`
so the sentinel is explicit.

### `let _ = self.process_loading()` silently swallows errors
Lines 939 and 1047. If `process_loading` fails, the TUI silently stays on
whatever screen it was on. Relates to uncovered `tui.network.error` spec rule.
At minimum, store the error and display it.

### `unwrap()` on `expanded.take()` (line 1245)
Safe at runtime because of the guard, but fragile under refactoring. Change to:
```rust
if let Some(expanded) = state.browse.expanded.take() {
```

### `debug_assert_eq!` in `render_detail` (line 1554)
Only fires in debug builds. If important, make it a compile-time guarantee.
If belt-and-suspenders, fine as-is.

## Test Quality

### `expect_test` opportunities
- `collect_changes_*` tests: manually assert individual fields of `AddChange`.
  Snapshot the `changes` vector instead (needs `Debug` on `AddChange`/`CrateChange`).
- `detail_selectable_items_includes_all_sections`: checks 8 items by index with
  `matches!`. Snapshot `items.iter().map(|i| format!("{:?}", i))` instead.
- `build_installed_state_from_spec` and `build_expanded_pack_defaults_prechecked`:
  assert individual fields. Snapshot the full state.

### Missing `BatteryPackSummary` fixture helper
Many tests construct `BatteryPackSummary { name: "x-battery-pack".into(), ... }` inline.
Add `make_summary(short_name, version, desc)` to match existing helpers.

### Repeated `if let Screen::X(...) else { panic!() }` pattern (~20 times)
Add helpers like `unwrap_add_screen(app: &App) -> &AddScreen`.

## Remaining Action Items

### High priority (spec gaps)
- [ ] `tui.installed.dep-kind`: Either implement dep-kind UI or explicitly defer the spec rule
- [ ] `tui.installed.show-state` / dep_kind: `CrateEntry` doesn't store dep_kind; `version_info()` doesn't display it

### Medium priority (code quality)
- [ ] Stop swallowing `process_loading()` errors with `let _ =`

### Low priority (polish)
- [ ] Use `expect_test` snapshots for `collect_changes` and `selectable_items` tests (derive `Debug` on `AddChange`/`CrateChange`)
- [ ] Consider `Option<Screen>` or `Screen::Empty` to avoid dummy sentinel in `mem::replace`

### Spec fixes (non-additive rules)
- [ ] Fold hidden-dep exceptions into parent listing rules (`tui.installed.list-crates`, `tui.browse.detail`)
- [ ] Make `tui.nav.exit` explicit about which exit paths apply changes
- [ ] Clarify toggle-crate vs features interaction in one rule

## Completed

- [x] Extract `wrapping_nav(index, count, forward)` helper — replaced 3 struct impls (DetailScreen, InstalledState, ExpandedPack)
- [x] Eliminate `PendingAction::OpenCratesIo` — folded into `OpenUrl`, callers construct URL
- [x] Add `focused_field_mut()` and `focused_field_len()` on `FormScreen` — replaced 5 dispatch sites + render_form cursor calc
- [x] Extract `bp_summary_list_item()` — shared by `render_list` and `render_add_browse`
- [x] Fix `expanded.take().unwrap()` — replaced with `if let Some(expanded) = take()`
- [x] Add `make_summary(short_name, version, desc)` test helper — replaced 7 inline `BatteryPackSummary` constructions
- [x] Add `unwrap_add_screen(app)` test helper — replaced 13 `if let Screen::Add ... else panic` patterns
