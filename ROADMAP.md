# Roadmap

Current: **v0.9.0** · Future: **v1.0.0** (API freeze — no code changes planned).

Eight feature milestones have shipped as minor-version releases;
patch releases have handled internal refactors, documentation,
and safety fixes. v0.9 closes the last open RFC 001 deferral
(ItemTree drag-and-drop, RFC 002). v1.0 freezes the API surface
that now includes drag-and-drop for both widgets.

## Status at a glance

| Version  | Status    | Theme                                                      |
| ---      | ---       | ---                                                        |
| `0.1.0`  | ✅ shipped | Initial release — basic tree, lazy loading, three filters. |
| `0.2.0`  | ✅ shipped | Per-path selection, keyboard nav, pluggable executor.      |
| `0.3.0`  | ✅ shipped | Multi-select (Shift/Ctrl-click, range extension).          |
| `0.4.0`  | ✅ shipped | Drag-and-drop between nodes.                               |
| `0.5.0`  | ✅ shipped | Parallel pre-expansion (`with_prefetch_limit`).            |
| `0.6.0`  | ✅ shipped | Incremental search with real-time filtering.              |
| `0.6.1`  | ✅ shipped | Prefetch safety valve (`.git` / `node_modules` / …).       |
| `0.7.0`  | ✅ shipped | Custom icon themes via an `IconTheme` trait.               |
| `0.7.1`  | ✅ shipped | Dependencies updated.                                      |
| `0.7.2`  | ✅ shipped | Framework-agnostic design documents (`docs/design/`).      |
| `0.8.0`  | ✅ shipped | Generic item tree — `ItemTree<T>` (RFC 001).               |
| `0.9.0`  | ✅ shipped | Drag-and-drop for `ItemTree<T>` (RFC 002).                 |
| `1.0.0`  | 🎯 next    | API freeze release. No code changes from `0.9.x`.          |

Patch releases (internal refactors, docs): `0.4.1`, `0.4.2`,
`0.6.2`, `0.6.3`. Summaries below.

---

## Shipped — feature releases

### v0.1.0 — Initial release ✅
The baseline: a lazy-loading directory tree with three
`DirectoryFilter` modes, Unicode-symbol icons by default with
an `icons` feature flag for real lucide glyphs, per-scan
generation tags to drop stale results after collapse/re-expand,
and the `async` scan path that keeps the UI thread unblocked.

### v0.2.0 — Per-path selection, keyboard nav, pluggable executor ✅
Added per-path selection tracking so filter changes preserve
the current selection; keyboard navigation (arrow keys,
`Home` / `End`, `Enter`, `Space`); and the `ScanExecutor` trait
so apps with their own blocking-task pool (`tokio`, `smol`,
`rayon`, …) can skip the per-expansion `std::thread::spawn`
default.

### v0.3.0 — Multi-select ✅
See [CHANGELOG](CHANGELOG.md#030--2026-04-23). Adds
`SelectionMode::{Replace, Toggle, ExtendRange}`, three new
accessors on `DirectoryTree`, Shift-arrow and Space-toggle
keyboard bindings, and the `multi_select` example.

### v0.4.0 — Drag-and-drop between nodes ✅
See [CHANGELOG](CHANGELOG.md#040--2026-04-24). Adds the `Drag`
and `DragCompleted` event variants, a `DragMsg` state-machine
enum, drop-target highlighting in the built-in view, multi-item
drag that preserves the current selection during the gesture,
deferred-selection so clicks on multi-selected rows don't
collapse the set, an Escape-to-cancel keybind, and the
`drag_drop` example that performs `fs::rename` on drop.

### v0.5.0 — Parallel pre-expansion ✅
See [CHANGELOG](CHANGELOG.md#050--2026-04-24). Opt-in via
`DirectoryTree::with_prefetch_limit(N)`: when a user expands a
folder, the widget speculatively issues parallel scans for up
to `N` of its folder-children so clicking any of them becomes
instant. One level deep only (no cascade). Respects
`max_depth`. `0` (default) disables prefetch entirely and
preserves v0.4 behaviour exactly.

### v0.6.0 — Incremental search ✅
See [CHANGELOG](CHANGELOG.md#060--2026-04-24). Apps call
`tree.set_search_query(...)` and the widget narrows rendering
to rows whose basenames match (case-insensitive substring) plus
every ancestor of every match. Selection survives search. New
`examples/search.rs`. One known limitation documented:
click-to-expand during search does not escape the filter; clear
the query first to explore.

### v0.7.0 — Custom icon themes via a trait ✅
See [CHANGELOG](CHANGELOG.md#070--2026-04-24). Introduces the
`IconTheme` trait (object-safe, `Send + Sync + Debug`), the
`IconRole` enum (`#[non_exhaustive]`), and the `IconSpec` data
struct. Two stock themes: `UnicodeTheme` (always available) and
`LucideTheme` (behind `icons` feature). New
`DirectoryTree::with_icon_theme(Arc<dyn IconTheme>)` builder
and `examples/icon_theme.rs`. The `icons` feature's purpose
shrinks to "ship lucide TTF + preset" — apps that plug in their
own theme can turn it off for a slimmer binary.

### v0.7.1 — Dependencies updated ✅

### v0.7.2 — Framework-agnostic design documents ✅
See [CHANGELOG](CHANGELOG.md#072--2026-06-07). Five Markdown
documents in `docs/design/` specify the widget's core design,
data model, state machine, feature contracts, and a Dioxus
porting guide. Written to support development of
`dioxus-swdir-tree`. No code or test changes.

## Shipped — safety patches

### v0.6.1 — Prefetch safety valve ✅
See [CHANGELOG](CHANGELOG.md#061--2026-04-24). The v0.5
prefetch machinery now refuses to speculatively scan
directories whose basenames appear in a configurable skip list.
Default covers `.git`, `.hg`, `.svn`, `node_modules`,
`__pycache__`, `.venv`, `venv`, `target`, `build`, `dist`.
Exact-basename match, ASCII case-insensitive. New
`with_prefetch_skip(iter)` builder and public
`DEFAULT_PREFETCH_SKIP` constant. Skip applies only to
prefetch — explicit user clicks still expand any folder.

## Shipped — tooling and documentation

### v0.4.1 — Internal source-layout refactor
See [CHANGELOG](CHANGELOG.md#041--2026-04-24). Seven inline
`#[cfg(test)] mod tests { ... }` blocks moved to sibling
`<module>/tests.rs` files; `update.rs` split into a thin
dispatcher plus four per-event handler submodules. No behaviour
or API changes.

### v0.4.2 — Test-layout refactor
See [CHANGELOG](CHANGELOG.md#042--2026-04-24). Two large
integration binaries split into 12 themed files plus a shared
`tests/common/mod.rs` helper. Same 100 tests, same names, same
behaviour.

### v0.6.2 — Documentation restructure
See [CHANGELOG](CHANGELOG.md#062--2026-04-24). `README.md`
shrank from ~500 to ~140 lines; 10 topic pages moved under
`docs/`; `ARCHITECTURE.md` and `DEVELOPMENT.md` relocated into
`docs/`.

### v0.6.3 — Documentation reorganization
See [CHANGELOG](CHANGELOG.md#063--2026-04-24). `docs/` now
organized by reader intent into three subfolders — `guide/`
(task-oriented), `reference/` (lookup), `internals/`
(architecture + dev). All filenames unified to
lowercase-kebab-case; two renamed for clarity (`executor.md` →
`custom-executor.md`, `keyboard.md` →
`keyboard-navigation.md`).

---

## Shipped — tooling and documentation

### v0.8.0 — Generic item tree (`ItemTree<T>`) ✅
See [CHANGELOG](CHANGELOG.md#080--2026-06-09). Implements
RFC 001. Adds `ItemTree<T>` with `NodeId(u64)` identity,
key-based diffing via `set_tree`, and the same keyboard /
multi-select / search / icon-theme surface as `DirectoryTree`.
No async I/O. Motivated by the `layered` Markdown editor's
section-outline use case.

### v0.9.0 — Drag-and-drop for `ItemTree<T>` ✅
See [CHANGELOG](CHANGELOG.md#090--2026-06-09). Implements
RFC 002. Adds opt-in (`with_drag_and_drop(true)`) drag-and-drop
reorder/nest to `ItemTree<T>` via a `Before`/`Into`/`After`
drop-position model, resolving the deferral in RFC 001 [D10].
Validity is checked via an O(depth) parent-map chain walk
(snapshotted once at drag start). Includes Escape-to-cancel,
deferred-selection, 27 new tests, and a worked example.

## Next: v1.0.0

With v0.9.0 shipped, every roadmap item originally planned for
v1.0 has landed. The 1.0 release is an API-freeze marker, not a
new feature release: it takes whatever is at v0.9.x and stamps
it as the stable surface.
