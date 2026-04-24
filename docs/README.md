# iced-swdir-tree documentation

Full reference for the widget. The
[top-level README](../README.md) covers the hero pitch, a
30-second quick start, and design notes — land here when you
need depth on a specific topic.

The documentation is organized by what you're trying to do:

- **[`guide/`](guide/)** — you want to *build something*.
  Task-oriented pages with code you can paste into your app.
- **[`reference/`](reference/)** — you want to *look up a fact*.
  The feature list; the event enum; short answers.
- **[`internals/`](internals/)** — you want to *understand how
  the widget works* or contribute. Architecture deep-dive and
  how to run the test matrix.

## Guide

Building something specific? Start here.

- [Configuration](guide/configuration.md) — builder methods,
  runtime mutators, accessors.
- [Multi-select](guide/multi-select.md) — `SelectionMode`,
  modifier handling, view-level click behaviour.
- [Drag-and-drop](guide/drag-and-drop.md) — the `DragCompleted`
  event, filesystem responsibility split, drag-state accessors.
- [Keyboard navigation](guide/keyboard-navigation.md) — the
  key-to-event table and subscription wiring.
- [Incremental search](guide/incremental-search.md) —
  `set_search_query`, match semantics, the
  selection-survives-search contract.
- [Parallel pre-expansion](guide/prefetch.md) —
  `with_prefetch_limit`, the `.git` / `node_modules` / `target`
  safety valve.
- [Custom scan executor](guide/custom-executor.md) —
  `ScanExecutor`, `ThreadExecutor`, when to swap.

## Reference

Looking something up.

- [Features](reference/features.md) — what the widget provides,
  at a glance.
- [Events](reference/events.md) — the `DirectoryTreeEvent` enum
  your `update()` handler consumes.

## Internals

Understanding or contributing.

- [Architecture](internals/architecture.md) — the module layout,
  generation-counter/cache mechanics, drag state machine,
  prefetch cascade-prevention trick, search-visibility walker.
- [Development](internals/development.md) — running the test
  matrix locally and extending it.

## Release notes

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

Run any of them with `cargo run --example <n>`.
