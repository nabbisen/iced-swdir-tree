# Features

A quick tour of what the widget provides. Each bullet links to a
dedicated reference page with concrete examples.

- **Multi-select** with Shift/Ctrl-click and Shift-arrow range
  extension. A per-path authoritative set survives filter changes
  and subtree reloads; see [Multi-select](multi-select.md).
- **Drag-and-drop between nodes.** Drag one or more selected
  paths onto another folder; the widget emits a
  `DragCompleted { sources, destination }` event and the app
  performs the actual move/copy/upload/whatever. The widget
  performs no filesystem operations itself. See
  [Drag-and-drop](drag-and-drop.md).
- **Parallel pre-expansion.** Opt into `with_prefetch_limit(N)`
  and the widget will speculatively scan the first `N`
  folder-children of any folder the user expands, in parallel
  via the executor, so clicking any of them is instant. One
  level deep only (no cascade). See
  [Parallel pre-expansion](prefetch.md).
- **Incremental search.** `tree.set_search_query(q)` narrows the
  visible rows to basename-substring matches plus their ancestor
  chain, so users see tree context alongside their hits.
  Selection survives the filter. See
  [Incremental search](search.md).
- **Lazy loading.** Only the root is created eagerly; child
  folders are scanned on first expand.
- **Non-blocking.** Directory traversal runs on a worker thread
  through `iced::Task::perform`; the UI thread never stalls on
  disk I/O. Plug in your own executor (`tokio`, `smol`, etc.)
  via [`with_executor`](executor.md) if you don't want the
  per-expansion thread-spawn default.
- **Three display filters.** `FoldersOnly`, `FilesAndFolders`
  (default), `AllIncludingHidden`. Filter changes are applied
  from an in-memory cache, so switching is instant — no re-scan.
  Expansion state and selection survive the swap. See
  [Configuration](configuration.md).
- **Keyboard navigation.** Arrow keys, `Home`/`End`, `Enter`,
  `Space`, `←`/`→`, plus Shift-modified variants for range
  extension and `Escape` to cancel a drag — see
  [Keyboard navigation](keyboard.md).
- **Stale-result handling.** Every scan carries a generation
  counter, so a collapse/re-expand cycle safely discards
  in-flight results from the cancelled round-trip.
- **Error tolerance.** Permission denials, missing paths, and
  symlink cycles are surfaced as per-node errors that the view
  greys out — no panics, no UI freezes.
- **Optional lucide icons.** Disabled by default; enable the
  `icons` feature to pull in real vector glyphs. The public API
  is identical in both modes. See
  [Quick start · Icons](../README.md#quick-start).
- **Cross-platform.** Hidden-file detection follows OS
  conventions: dotfile on Unix, `HIDDEN` attribute plus dotfile
  fallback on Windows, dotfile elsewhere.
