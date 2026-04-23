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
    node.rs                    # TreeNode, LoadedEntry, TreeCache
    update.rs                  # State machine for update()
    view.rs                    # Render function for view()
    walker.rs                  # async scan wrapper + normalization
```

The public API is intentionally small. The internal layering
separates state ownership (`directory_tree.rs`), events (`message.rs`),
state transitions (`update.rs`), rendering (`view.rs`), data access
(`walker.rs`), blocking-work dispatch (`executor.rs`), and keyboard
translation (`keyboard.rs`), which makes room for the v0.4+ roadmap
items (multi-select, drag-and-drop, custom icon themes) without
touching the widget surface.

## Selection cursor

Selection is stored by path on `DirectoryTree::selected_path` — the
canonical cursor. The per-node `TreeNode::is_selected` flag is a
view-layer cache that the update layer keeps in sync. This lets
filter changes and subtree reloads preserve the user's selection
even when the selected node's in-memory representation is
reconstructed from scratch.

## Scan dispatch

`walker::scan` is runtime-agnostic: it produces an `iced::Task`
backed by a `ScanFuture`, itself obtained from
`Arc<dyn ScanExecutor>`. The default `ThreadExecutor` spawns one
`std::thread` per scan, which is correct but slightly wasteful for
apps that already run a blocking-task pool. Those apps can
implement `ScanExecutor` (one method, `spawn_blocking`) and swap
it in at construction time via `DirectoryTree::with_executor`.
