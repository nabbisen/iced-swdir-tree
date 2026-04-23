# Roadmap

## v0.2 — v0.3

- Per-path selection tracking so filter changes preserve selection.
- Keyboard navigation (arrow keys, enter to toggle, space to select).
- Drive the scan through a pluggable async runtime shim so applications
  using `tokio` rather than smol don't pay for a thread-spawn per
  expansion.

## v0.4 — v0.6

- Multi-select (shift/ctrl-click).
- Drag-and-drop between nodes.
- Custom icon themes — swap lucide for your own icon set via a trait.

## v1.0 and beyond

- Incremental search with real-time filtering.
- Parallel pre-expansion of visible descendants.
- Plugin-style extension points (per-node decorators, custom context
  menus, etc.).
