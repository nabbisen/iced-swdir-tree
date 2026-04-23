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

## Remaining for v1.0

### v0.4.0 — Drag-and-drop between nodes
Let the user drag one or more selected paths onto another folder in
the same tree. Requires an `iced::event::listen` hook for mouse-drag
state and a drop-target overlay on folder rows. Surface as
`DirectoryTreeEvent::DragCompleted { sources, destination }` events
so the application decides what to do with the files (move, copy,
symlink, network upload, ...) — the widget never touches the
filesystem itself.

### v0.5.0 — Parallel pre-expansion of visible descendants
Expanding a folder issues one scan; expanding ten sub-folders
issues ten serial round-trips through the executor. A "pre-expand"
mode can fire N scans in parallel for everything visible but not yet
loaded, throttled by a configurable concurrency knob. Builds on the
`ScanExecutor` trait from v0.2 so `tokio`/`smol` users get native
parallelism for free.

### v0.6.0 — Incremental search with real-time filtering
A text-input filter on top of the tree: as the user types, rows
whose path doesn't match are hidden, but their ancestors stay
visible so the match's context is preserved. Reuses the v0.2
filter-rebuild machinery so selection and expansion state survive
the type-ahead.

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
