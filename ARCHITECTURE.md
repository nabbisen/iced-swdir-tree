# Architecture

```text
src/
  lib.rs                       # Re-exports and crate docs
  directory_tree.rs            # State struct and builder methods
  directory_tree/
    config.rs                  # DirectoryFilter, TreeConfig
    error.rs                   # Crate Error type
    executor.rs                # ScanExecutor trait, ThreadExecutor default
    icon.rs                    # Feature-gated icon renderer (lucide / text)
    keyboard.rs                # handle_key adapter + bindings
    message.rs                 # DirectoryTreeEvent + LoadPayload
    node.rs                    # TreeNode, LoadedEntry, TreeCache, VisibleRow
    selection.rs               # SelectionMode enum + from_modifiers
    update.rs                  # State machine for update()
    view.rs                    # Render function for view()
    walker.rs                  # async scan wrapper + normalization
```

The public API is intentionally small. The internal layering
separates state ownership (`directory_tree.rs`), events (`message.rs`),
state transitions (`update.rs`), rendering (`view.rs`), data access
(`walker.rs`), blocking-work dispatch (`executor.rs`), keyboard
translation (`keyboard.rs`), and selection modes (`selection.rs`),
which makes room for the remaining v1.0 roadmap items
(drag-and-drop, parallel pre-expand, incremental search, icon
themes) without touching the widget surface.

## Selection model (v0.3+)

Three fields on `DirectoryTree` encode the selected set:

| Field            | Role |
|------------------|------|
| `selected_paths: Vec<PathBuf>` | The authoritative set of selected paths. Order is not semantic. |
| `active_path: Option<PathBuf>` | The most recently touched path (last click, last Space-toggle, last `ExtendRange` target). `selected_path()` returns this — preserving the v0.2 single-select accessor semantics. |
| `anchor_path: Option<PathBuf>` | The pivot for `SelectionMode::ExtendRange`. Updated by `Replace` and `Toggle`; **not** updated by `ExtendRange` itself — successive Shift+clicks all extend from the same origin, matching Windows Explorer / macOS Finder / VS Code. |

The per-node `TreeNode::is_selected` flag is a view-layer cache
re-synced from `selected_paths` by
`DirectoryTree::sync_selection_flags()` after any mutation that
replaces node instances. This keeps filter changes and subtree
reloads lossless: every selected path survives because the cache
gets re-derived from the authoritative set.

### Mode semantics

`SelectionMode::from_modifiers(m)` maps iced `Modifiers` into one of
three variants:

- `Replace` (no modifier): clears the set, selects just the target.
  Updates both `active_path` and `anchor_path`.
- `Toggle` (Ctrl/Cmd): adds the target if absent, removes it if
  present. Updates both `active_path` and `anchor_path`.
- `ExtendRange` (Shift): replaces the set with every visible row
  between `anchor_path` and the target, in render order. Updates
  only `active_path`; the anchor stays put so further Shift+clicks
  remain anchored. Falls back to `Replace` semantics if no anchor
  is set or if either endpoint is not currently visible (filtered
  out, ancestor collapsed, not yet loaded).

### View-level click modifiers

iced 0.14's `button::on_press` can't observe modifier keys, so the
built-in view emits `SelectionMode::Replace` unconditionally.
Applications that want multi-select track modifier state separately
(via a `keyboard::listen()` subscription) and rewrite the mode in
their own update handler before forwarding to `tree.update` — the
`examples/multi_select.rs` example demonstrates the full pattern.
Keyboard bindings (`handle_key`) produce the right mode directly
because key events carry modifiers at press time.

## Scan dispatch

`walker::scan` is runtime-agnostic: it produces an `iced::Task`
backed by a `ScanFuture`, itself obtained from
`Arc<dyn ScanExecutor>`. The default `ThreadExecutor` spawns one
`std::thread` per scan, which is correct but slightly wasteful for
apps that already run a blocking-task pool. Those apps can
implement `ScanExecutor` (one method, `spawn_blocking`) and swap
it in at construction time via `DirectoryTree::with_executor`.
