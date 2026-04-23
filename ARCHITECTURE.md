# Architecture

```text
src/
  lib.rs                       # Re-exports and crate docs
  directory_tree.rs            # State struct and builder methods
  directory_tree/
    config.rs                  # DirectoryFilter, TreeConfig
    error.rs                   # Crate Error type
    icon.rs                    # Feature-gated icon renderer (lucide / text)
    message.rs                 # DirectoryTreeEvent + LoadPayload
    node.rs                    # TreeNode, LoadedEntry, TreeCache
    update.rs                  # State machine for update()
    view.rs                    # Render function for view()
    walker.rs                  # async scan wrapper + normalization
```

The public API is intentionally small. The internal layering
separates state ownership (`directory_tree.rs`), events (`message.rs`),
state transitions (`update.rs`), rendering (`view.rs`), and data access
(`walker.rs`), which makes room for the v0.4+ roadmap items
(multi-select, drag-and-drop, custom icon themes) without touching the
widget surface.
