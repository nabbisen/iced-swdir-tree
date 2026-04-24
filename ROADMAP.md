# Roadmap

All five items below are **required for v1.0** and are listed in
implementation order. Each lands as a discrete minor-version release
so downstream apps can upgrade one feature at a time.

## Shipped

### v0.2.0 — per-path selection, keyboard nav, pluggable executor
- Per-path selection tracking so filter changes preserve selection.
- Keyboard navigation (arrow keys, `Enter` to toggle, `Space` to
  re-emit — later changed to toggle in v0.3).
- Pluggable `ScanExecutor` so apps with their own blocking-task
  pool (`tokio`, `smol`, `rayon`, ...) skip the per-expansion
  `std::thread::spawn`.

### v0.3.0 — Multi-select (Shift/Ctrl-click) ✅
See [CHANGELOG](CHANGELOG.md#030--2026-04-23). Adds
`SelectionMode::{Replace, Toggle, ExtendRange}`, three new
accessors on `DirectoryTree`, Shift-arrow and Space-toggle
keyboard bindings, and the `multi_select` example.

### v0.4.0 — Drag-and-drop between nodes ✅
See [CHANGELOG](CHANGELOG.md#040--2026-04-24). Adds the `Drag`
and `DragCompleted` event variants, a `DragMsg` state-machine
enum, drop-target highlighting in the built-in view, multi-item
drag that preserves the current selection during the gesture,
deferred-selection so clicks on multi-selected rows don't collapse
the set, an Escape-to-cancel keybind, and the `drag_drop` example
that performs `fs::rename` on drop.

### v0.4.1 — Internal refactor ✅
See [CHANGELOG](CHANGELOG.md#041--2026-04-24). Pure file-layout
refactor: seven inline `#[cfg(test)] mod tests { ... }` blocks
moved to sibling `<module>/tests.rs` files, and `update.rs` split
into a thin dispatcher plus four per-event handler submodules.
No behaviour or API changes.

### v0.4.2 — Test-layout refactor ✅
See [CHANGELOG](CHANGELOG.md#042--2026-04-24). Pure `tests/`
reorganization: the two large integration binaries split into 12
themed files plus a shared `tests/common/mod.rs` helper. Same
100 tests, same names, same behaviour; smaller files.

### v0.5.0 — Parallel pre-expansion of visible descendants ✅
See [CHANGELOG](CHANGELOG.md#050--2026-04-24). Opt-in via
`DirectoryTree::with_prefetch_limit(N)`: when a user expands a
folder, the widget speculatively issues parallel scans for up to
`N` of its folder-children so clicking any of them becomes
instant. One level deep only (no cascade). Respects `max_depth`.
`0` (default) disables prefetch entirely and preserves v0.4
behaviour exactly.

### v0.6.0 — Incremental search with real-time filtering ✅
See [CHANGELOG](CHANGELOG.md#060--2026-04-24). Apps call
`tree.set_search_query(...)` and the widget narrows rendering to
rows whose basenames match (case-insensitive substring) plus
every ancestor of every match. Selection survives search. New
`examples/search.rs`. One known limitation documented:
click-to-expand during search does not escape the filter;
clear the query first to explore.

### v0.6.1 — Prefetch safety valve ✅
See [CHANGELOG](CHANGELOG.md#061--2026-04-24). The v0.5 prefetch
machinery now refuses to speculatively scan directories whose
basenames appear in a configurable skip list. Default covers
`.git`, `.hg`, `.svn`, `node_modules`, `__pycache__`, `.venv`,
`venv`, `target`, `build`, `dist`. Exact-basename match, ASCII
case-insensitive. New `with_prefetch_skip(iter)` builder and
public `DEFAULT_PREFETCH_SKIP` constant. Skip applies only to
prefetch — explicit user clicks still expand any folder.

## Remaining for v1.0

### v0.7.0 — Custom icon themes via a trait
Swap `lucide-icons` for another icon set (material, heroicons, app-
specific glyphs) via an `IconTheme` trait that returns the glyph
(and optional font) for each logical icon role (`folder-closed`,
`folder-open`, `file`, `symlink`, `error`). Keeps the `icons`
feature flag as a convenient default but removes the hard-coded
dependency.

## After v1.0

- View-layer virtualization — iced's `Scrollable` is fine through
  tens of thousands of rows; beyond that, a custom low-level widget
  that renders only on-screen rows would pay off.
- Per-node badge / decorator API — a trait app developers can
  implement to add e.g. git-status dots, file-size labels, or
  last-modified timestamps.
- Context-menu hooks (`on_right_click`-style events).
- Deeper prefetch cascade (configurable depth, global concurrency
  cap) — can land as a 0.5.x patch if demand materializes.
- Pluggable search matcher trait (regex / glob / fuzzy / full-path
  mode) — can land as a 0.6.x patch. The v0.6 defaults cover the
  common case; a trait seam would let apps opt into more.
- Opt-in "click-to-escape-search" behaviour for tree exploration
  while search is active — also a 0.6.x candidate.
