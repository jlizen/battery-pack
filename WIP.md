# WIP: TUI Testing

## Goal

Add test coverage for the 24 `tui.*` spec rules in `md/spec/tui.md`.
The TUI lives in `src/battery-pack/bphelper-cli/src/tui.rs` (~2000 lines).

## Architecture summary

The TUI is structured as:
- **State**: `App` holding a `Screen` enum (Loading, List, Detail, NewProjectForm, Add)
- **Input**: `handle_key()` produces an `Action` enum, then applies it — decoupled from rendering
- **Rendering**: Free functions (`render_list`, `render_detail`, `render_add`, etc.) take `&mut Frame`

All types are private to `tui.rs`, so tests go in a `#[cfg(test)] mod tests` block in that file.

## Approach — three tiers, in order

### Tier 1: State logic (pure, no terminal needed)

Construct state structs directly, call methods, assert on results.
Build test fixture helpers here that Tier 2 and 3 reuse.

| Spec rule | What to test | Key code |
|-----------|-------------|----------|
| `tui.installed.toggle-crate` | `InstalledState::toggle_selected()` flips `enabled` | `toggle_selected()` |
| `tui.installed.show-state` | `CrateEntry` carries enabled, dep kind, version | struct fields |
| `tui.installed.features` | Feature groups in `all_crates_with_grouping()` — toggling one crate doesn't affect others in different groups | `toggle_selected()` + group assertions |
| `tui.installed.hidden` | Already verified in bphelper-manifest | `resolve_for_features` filters hidden |
| `tui.browse.hidden` | Already verified in bphelper-manifest | same |
| `tui.nav.exit` | `collect_changes()` produces correct `AddChange` variants | `collect_changes()` |
| `tui.installed.dep-kind` | `CrateEntry` tracks dep kind from battery pack spec | struct construction |
| `tui.browse.add` | `build_expanded_pack()` creates correct entries with defaults pre-checked | `build_expanded_pack()` |
| `tui.browse.detail` | `DetailScreen::selectable_items()` includes crates, templates, examples | `selectable_items()` |

### Tier 2: Key handling (construct App, send keys, assert screen transitions)

Build `App` in a known `Screen` state (bypassing `process_loading`),
call `handle_key(KeyCode::...)`, assert resulting screen and state.

| Spec rule | What to test |
|-----------|-------------|
| `tui.nav.keyboard` | j/k, arrows, Enter, Space, Esc, q, Tab all do the right thing per screen |
| `tui.nav.cancel` | Esc/q from Add screen quits without applying changes |
| `tui.nav.exit` | Enter from Add screen with changes sets `changes` and `should_quit` |
| `tui.main.sections` | (partially) — screen transitions cover that List, Detail, Add screens exist |
| `tui.browse.search` | `/` enters search mode, typing updates input, Enter triggers search |
| `tui.browse.add` | Enter on browse item → expanded pack → Enter confirms → moves to Installed tab |
| `tui.installed.toggle-crate` | Space toggles in Add/Installed screen |
| `tui.installed.list-crates` | (partially) — navigation covers that entries exist and are selectable |

### Tier 3: Rendering (Buffer assertions, selective)

Render specific screens into `Buffer::empty()`, assert visible content.
Only for spec rules that make claims about what must be displayed.

| Spec rule | What to test |
|-----------|-------------|
| `tui.main.no-project` | `render_add_installed` with empty packs shows "No battery packs installed" |
| `tui.installed.list-packs` | Pack headers show name and version |
| `tui.installed.list-crates` | Crate entries show checkbox, name, version info |
| `tui.installed.show-state` | Enabled crates show `[x]`, disabled show `[ ]` |
| `tui.browse.list` | Browse list shows name, version, description |

### Deferred: I/O-dependent rules

These need mocking or extraction and aren't worth the refactor cost right now.

| Spec rule | Why deferred |
|-----------|-------------|
| `tui.main.always-available` | Requires running the binary — E2E territory |
| `tui.main.context-detection` | `process_loading` calls `load_installed_packs` with real filesystem |
| `tui.network.non-blocking` | Would need async mock of fetch functions |
| `tui.network.error` | Same — fetch functions are called directly in `process_loading` |
| `tui.new.template-list` | Depends on real battery pack data |
| `tui.new.create` | Shells out to `cargo bp new` |

## Progress

- [ ] Tier 1: State logic tests
- [ ] Tier 2: Key handling tests
- [ ] Tier 3: Rendering tests (selective)
- [ ] Tag all new tests with `[verify tui.*]`
- [ ] Final review: verify tags are honest (would the test fail if the spec were violated?)
