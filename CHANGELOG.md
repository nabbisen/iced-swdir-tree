# Changelog

All notable changes to `iced-swdir-tree` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the crate follows [Semantic Versioning](https://semver.org/).

## [0.3.0] — 2026-04-23

Delivers the first of the five v1.0-required roadmap items:
**multi-select** (Shift/Ctrl-click, Shift-arrow, Space-toggle).

### Added

- **Multi-select via [`SelectionMode`].** New public enum with three
  variants — `Replace` (default / plain click), `Toggle`
  (Ctrl/Cmd-click), and `ExtendRange` (Shift-click). A
  `from_modifiers(Modifiers)` convenience maps an iced `Modifiers`
  value to the right mode.
- **Three new accessors on `DirectoryTree`:**
  - `selected_paths() -> &[PathBuf]` — the full selected set.
  - `anchor_path() -> Option<&Path>` — the pivot for `ExtendRange`
    (not moved by `ExtendRange` itself, matching Explorer/Finder).
  - `is_selected(&Path) -> bool` — membership check.
- **Keyboard multi-select:**
  - `Shift + ↑/↓/Home/End` extend the selected range.
  - `Space` and `Ctrl+Space` toggle the active path in/out of the
    set (changed from v0.2 — see BREAKING below).
- **`examples/multi_select.rs`** — full working demo showing the
  modifier-tracking pattern and a live multi-selection status bar.

### Changed — BREAKING

- `DirectoryTreeEvent::Selected(PathBuf, bool)` →
  `DirectoryTreeEvent::Selected(PathBuf, bool, SelectionMode)`. Existing
  apps only need to add `SelectionMode::Replace` (or `_` in
  pattern-matches where mode is irrelevant). Migration is a one-line
  sed per match site.
- Internal state: `selected_path: Option<PathBuf>` is replaced by three
  fields (`selected_paths: Vec<PathBuf>`, `active_path: Option<PathBuf>`,
  `anchor_path: Option<PathBuf>`). The public `selected_path()` accessor
  still returns the last-touched path, preserving v0.2 semantics for
  single-select callers — no change required for apps that don't
  care about multi-select.
- `handle_key` now uses the `modifiers` argument (ignored in v0.2).
  `Space` is now `Toggle` instead of "re-emit current selection" — the
  v0.2 behaviour was rarely useful and the new one matches VS Code,
  Finder, and Explorer.

### View-level click behaviour

The built-in view emits `Selected(path, is_dir, SelectionMode::Replace)`
on every click because iced 0.14's `button::on_press` callback cannot
observe modifier keys. Applications that want multi-select track
modifier state themselves via a `keyboard::listen()` subscription and
rewrite the mode before forwarding — see `examples/multi_select.rs`.
This will become unnecessary if a future iced release exposes
modifiers at press time.

### Test coverage

- 70 tests pass (up from 52 in v0.2):
  - 41 unit tests (was 27): + 5 `SelectionMode`, + 3 shift/ctrl
    keyboard binding tests, + 6 multi-select state-machine tests.
  - 10 integration tests (unchanged count; all migrated to the
    3-arg `Selected` form).
  - 18 tree-layer tests (was 14): + 4 new v0.3 multi-select
    integration tests over a real filesystem (toggle builds up a
    set, range covers siblings, filter change preserves every
    selected path, `selected_path()` tracks the last action target).
  - 1 compile-only doctest.

## [0.2.0] — 2026-04-23

The v0.2 release knocks out every item on the v0.2 — v0.3 roadmap
and bumps the `swdir` dependency from 0.9 to 0.10.

### Added

- **Per-path selection tracking.** Selection is now stored as a
  `selected_path: Option<PathBuf>` on `DirectoryTree`, with the
  per-node `is_selected` flag re-synced by the update layer. The
  cursor survives filter changes and subtree re-loads, and `set_filter`
  re-applies selection against the rebuilt node graph automatically.
- **Expansion state also survives filter changes.** `set_filter`'s
  rebuild now carries whole child subtrees over path-keyed instead
  of reconstructing empty nodes — an opened folder stays open, an
  already-loaded descendant stays loaded.
- **Keyboard navigation.** New `DirectoryTree::handle_key` adapter
  that turns an `iced::keyboard::Key` + `Modifiers` pair into an
  appropriate `DirectoryTreeEvent`. Bindings: `↑`/`↓` move along
  visible rows, `Home`/`End` jump to first/last, `Enter` toggles
  folders, `Space` re-emits the current selection, `←` collapses
  or moves to parent, `→` expands or moves to first child. The
  widget stays focus-neutral — the app decides when the tree
  "has focus" and pipes keypresses in.
- **Pluggable scan executor.** New `ScanExecutor` trait with
  `ScanJob` / `ScanFuture` type aliases and a default
  `ThreadExecutor` implementation. Apps that already have a
  blocking-task pool (tokio, smol, rayon, ...) can implement the
  trait and plug in via `DirectoryTree::with_executor(Arc<dyn
  ScanExecutor>)`. Default behaviour is unchanged — one
  `std::thread::spawn` per expansion via `ThreadExecutor`.
- New `examples/keyboard_nav.rs` demonstrating the subscription
  pattern.

### Changed

- **Dependency**: `swdir = "0.9"` → `swdir = "0.10"`. iced-swdir-tree
  only uses `swdir::scan_dir`, `DirEntry`, and `ScanError`, all of
  which are unchanged across swdir's 0.10 cleanup release — no API
  impact on downstream apps.
- `TreeNode::find_selected` removed (was crate-private; selection is
  now cursor-based, so the whole-tree walk is no longer needed).

### Resolved

- The v0.1 known limitation "filter change drops per-node
  selection state" is gone. Selection and expansion both survive
  runtime filter changes now.

### Test coverage

- 52 tests pass (up from 25 in v0.1):
  - 27 unit tests (14 original + 13 for the new keyboard module).
  - 10 integration tests (one updated for the new selection-preserving
    semantics).
  - 14 tree-layer tests (10 original + 2 rewritten for v0.2
    semantics + 4 new v0.2-specific coverage of executor swap and
    subtree-preservation-across-filter-change).
  - 1 compile-only doctest.

## [0.1.0] — Unreleased

Initial release.

### Added

- `DirectoryTree` widget with lazy, asynchronous loading via
  `iced::Task::perform` + `swdir::scan_dir`.
- `DirectoryFilter` with three modes: `FoldersOnly`,
  `FilesAndFolders` (default), `AllIncludingHidden`.
- `DirectoryTreeEvent` with `Toggled`, `Selected`, and opaque
  `Loaded` variants.
- `TreeNode`, `LoadedEntry`, `TreeCache` state types.
- `TreeConfig` for per-tree configuration (root, filter, depth limit).
- Crate-level `Error` type (clone-able; wraps `io::ErrorKind` + message).
- Builder API: `new`, `with_filter`, `with_max_depth`, `set_filter`.
- Read accessors: `filter`, `max_depth`, `root_path`, `selected_path`.
- `icons` feature flag, gating a lucide-icons-based glyph renderer.
  Public API is identical with and without the feature.
- Per-OS hidden-file detection: dotfile on Unix, `HIDDEN` attribute with
  dotfile fallback on Windows, dotfile elsewhere.
- Stale-result detection via a per-scan generation counter.
- Permission-denied and missing-path handling: surfaced as
  `TreeNode::error`, greyed out in the view, never panicking.
- Sorted output: directories first, files second, each group sorted
  case-insensitively.
- 25 tests (14 unit + 10 integration + 1 compile-only doc-test)
  covering every filter mode, expand/collapse round-trips, selection,
  stale-result rejection, permission-denied, and nonexistent paths.
- Two examples: `basic` (Unicode-symbol fallback) and `with_icons`
  (lucide-icons feature).

### Known limitations (at v0.1; resolved in v0.2)

- **Filter change drops per-node selection state.** *Fixed in v0.2 —
  selection is now stored by path on the widget, not by flag on
  rebuilt nodes.*
- **Multi-select, drag-and-drop, and search are not implemented.** See
  ROADMAP.
- **View-layer virtualization is delegated to iced's `Scrollable`.**
  Very large trees (hundreds of thousands of loaded nodes) may show
  layout-pass slowdowns. A future custom low-level widget could narrow
  the rendered slice to only the on-screen rows.
