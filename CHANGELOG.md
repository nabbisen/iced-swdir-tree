# Changelog

All notable changes to `iced-swdir-tree` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the crate follows [Semantic Versioning](https://semver.org/).

## [0.4.0] — 2026-04-24

Delivers the second of the five v1.0-required roadmap items:
**drag-and-drop between nodes**. The widget tracks drag gestures
internally and emits a `DragCompleted { sources, destination }`
event on successful drop; the application decides what to do with
the paths (move / copy / symlink / upload / anything). The widget
performs no filesystem operations itself.

### Added

- **New event variants on `DirectoryTreeEvent`:**
  - `Drag(DragMsg)` — opaque drag-machinery event. Apps route it
    back through `tree.update()` unchanged, exactly like `Loaded`.
  - `DragCompleted { sources: Vec<PathBuf>, destination: PathBuf }`
    — fires when the user releases the mouse over a valid folder
    row. Apps observe this event to perform the actual filesystem
    operation.
- **New public `DragMsg` enum** — re-exported from the crate root.
  Variants: `Pressed(PathBuf, bool)`, `Entered(PathBuf)`,
  `Exited(PathBuf)`, `Released(PathBuf)`, `Cancelled`. Generally
  constructed by the widget itself; apps only need to match on
  `Cancelled` if they want to force-cancel a drag.
- **Three new accessors on `DirectoryTree`:**
  - `is_dragging() -> bool`
  - `drop_target() -> Option<&Path>` — the currently-hovered valid
    folder target, or `None` when over empty space / files / self / a
    descendant of a source.
  - `drag_sources() -> &[PathBuf]` — paths being dragged.
- **Drag-aware drop-target highlight** in the built-in view. The
  hovered folder row paints with the theme's `success.weak`
  background and a `success.strong` outline.
- **Deferred-selection pattern.** Mouse-down on a row no longer
  immediately collapses a multi-selection down to that row. If the
  user releases on the same row the widget emits a delayed
  `Selected(path, is_dir, Replace)`; if they release on a different
  valid folder it emits `DragCompleted` instead. This matches
  Explorer / Finder behaviour — you can drag a multi-selection
  without losing it.
- **Multi-item drag.** Pressing on a row that's already in the
  selection drags the whole selected set; pressing on an unselected
  row drags only that row.
- **`Escape` cancels an in-flight drag.** The widget's built-in key
  handler produces `Drag(Cancelled)` when `Escape` is pressed while
  a drag is active. When no drag is active, `Escape` stays unbound
  so apps can still use it for their own UI.
- **`examples/drag_drop.rs`** — a complete working example that
  performs `fs::rename` on `DragCompleted` and refreshes affected
  folders. Includes modifier tracking for multi-select, live
  drag-preview status bar, and a safe default scratch directory
  under the OS temp dir so you can experiment without data risk.

### Changed

- **Row hit-testing is now a `mouse_area` around a styled
  `container`** rather than a `button`. This was required to
  observe mouse-down (for drag start) separately from mouse-up
  (for click vs. drag disambiguation) — iced 0.14's
  `button::on_press` fires only on click-completion. The
  user-visible row appearance is unchanged for the normal and
  selected states (the container style reproduces `button::text`
  and `button::primary` via the theme's `palette.primary.base`).
  Files still align with folders because the caret is still its
  own button on directory rows and a fixed-size `Space` on files.
- **Known trade-off:** non-selected rows lose the native hover
  glow that `button::text` provided, because `mouse_area` + plain
  `container` don't track hover state. Selected rows still paint
  with the primary-base background. A future version may add an
  explicit hover style; the omission is deliberate for now in
  favour of shipping drag-and-drop soon.
- **Known behaviour:** if the user releases the mouse outside the
  tree (over a scrollbar, empty space, another window), the drag
  state stays active until they press `Escape` or the app forces
  a `Cancelled`. A fix via an `iced::event::listen` subscription
  is possible but deliberately deferred — this matches what most
  native file browsers do.

### Public API — no breaking changes to existing code paths

- `DirectoryTreeEvent` is non-exhaustive and gains two new variants.
  Apps that exhaustively `match` on it without a `_` arm need to
  add arms for `Drag` (route back to `tree.update`) and
  `DragCompleted` (the app's own move/copy/etc. logic). Most apps
  use `.map(MyMessage::Tree)` and don't need changes.

### Tests

- **100 total (up from 70):** 60 unit + 10 + 29 integration + 1
  doctest. New coverage includes 6 `DragState::is_valid_target`
  unit tests (file rejected, self rejected, descendant rejected,
  sibling accepted, parent accepted, prefix-but-not-ancestor
  accepted), 12 state-machine transition tests for `on_drag`, 2
  keyboard tests for Escape-cancels / Escape-unbound-without-drag,
  and 11 integration tests driving the full public API against a
  real temp filesystem.

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
