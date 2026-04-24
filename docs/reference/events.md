# Events

The widget emits `DirectoryTreeEvent`:

- `Toggled(PathBuf)` — the user clicked the caret on a folder.
- `Selected(PathBuf, bool, SelectionMode)` — the user selected a
  row; `bool` is `true` for directories, `false` for files, and
  the [`SelectionMode`](../guide/multi-select.md) controls how the event
  composes with any existing selection.
- `Drag(DragMsg)` — internal drag-state messages. Your app
  typically routes these straight back into `update()` without
  inspecting them. See [Drag-and-drop](../guide/drag-and-drop.md).
- `DragCompleted { sources, destination }` — a successful drop.
  Your app reacts by performing the move/copy/etc.
- `Loaded(LoadPayload)` — internal; a pending scan completed.
  Parent applications route it straight back into `update()`
  without inspecting it.
