# iced-swdir-tree documentation

Full reference for the widget. The [top-level README](../README.md)
covers the hero pitch, a 30-second quick start, and design notes
— land here when you need depth on a specific topic.

## Topic index

- [Features](features.md) — what the widget provides, at a
  glance.
- [Configuration](configuration.md) — builder methods, runtime
  mutators, accessors.
- [Events](events.md) — the `DirectoryTreeEvent` enum your
  `update()` handler consumes.
- [Multi-select](multi-select.md) — `SelectionMode`, modifier
  handling, view-level click behaviour.
- [Drag-and-drop](drag-and-drop.md) — the `DragCompleted`
  event, filesystem responsibility split, drag-state accessors.
- [Parallel pre-expansion](prefetch.md) — `with_prefetch_limit`,
  the `.git` / `node_modules` / `target` safety valve.
- [Incremental search](search.md) — `set_search_query`, match
  semantics, the selection-survives-search contract.
- [Keyboard navigation](keyboard.md) — the key-to-event table
  and subscription wiring.
- [Custom scan executor](executor.md) — `ScanExecutor`,
  `ThreadExecutor`, when to swap.

## Deeper references

- [ARCHITECTURE.md](ARCHITECTURE.md) — the module layout,
  generation-counter/cache mechanics, drag state machine,
  prefetch cascade-prevention trick, search-visibility walker.
- [DEVELOPMENT.md](DEVELOPMENT.md) — running the test matrix
  locally and extending it.
- [CHANGELOG](../CHANGELOG.md) — per-release notes with
  breaking-change callouts.
- [ROADMAP](../ROADMAP.md) — what's shipped and what's planned
  for v1.0 and after.

## Examples

Working apps live in [`../examples/`](../examples/):

- `keyboard_nav.rs` — single-select navigation with the full
  key-binding surface.
- `multi_select.rs` — modifier-aware selection with a live
  selection-count status bar.
- `drag_drop.rs` — `fs::rename` on drop with post-move refresh.
- `search.rs` — text-input + tree + match-count status bar.

Run any of them with `cargo run --example <name>`.
