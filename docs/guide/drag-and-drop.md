# Drag-and-drop

The widget tracks drag gestures internally and emits a
`DragCompleted { sources, destination }` event when the user
releases over a valid folder. The widget **does not** touch the
filesystem — your app reacts to `DragCompleted` and performs the
actual move / copy / upload / whatever, then re-scans affected
folders so the view reflects the new layout.

```rust,ignore
match message {
    Message::Tree(DirectoryTreeEvent::DragCompleted {
        sources,
        destination,
    }) => {
        for src in &sources {
            if let Some(name) = src.file_name() {
                let _ = std::fs::rename(src, destination.join(name));
            }
        }
        // Refresh the destination and each source's parent so the
        // tree picks up the new layout. A collapse+re-expand via
        // two Toggled events is the simplest invalidation.
        let mut tasks = vec![];
        let mut refresh: std::collections::HashSet<PathBuf> = Default::default();
        refresh.insert(destination);
        for s in &sources {
            if let Some(p) = s.parent() { refresh.insert(p.into()); }
        }
        for p in refresh {
            tasks.push(Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p.clone()))));
            tasks.push(Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p))));
        }
        Task::batch(tasks)
    }
    Message::Tree(event) => self.tree.update(event).map(Message::Tree),
    // ...
}
```

Pressing the mouse on a row that's already in the selection
drags the whole selected set; pressing on an unselected row
drags only that row. `Escape` cancels an in-flight drag. If the
mouse is released outside the tree (or over empty space), the
drag stays active until `Escape` or an app-initiated cancel —
deliberately chosen to match native file-browser behaviour.

Three read-only accessors let your UI reflect drag state:

```rust,ignore
tree.is_dragging();      // bool
tree.drag_sources();     // &[PathBuf]
tree.drop_target();      // Option<&Path> — hovered valid folder
```

See [`examples/drag_drop.rs`](../../examples/drag_drop.rs) for a
complete working app with `fs::rename` on drop, post-move
refresh, and a live drag-preview status bar.

---

## `ItemTree<T>` — reorder and nest (v0.9.0)

`ItemTree<T>` supports drag-and-drop reorder/nest via the same
deferred-selection, Escape-cancel model as `DirectoryTree`, with
one key difference: the drop is described as a **position relative
to a target node** rather than "drop into a folder."

### Enabling

Drag-and-drop is **off by default** on `ItemTree`. Enable it with
the builder:

```rust,ignore
let tree: ItemTree<MyNode> = ItemTree::new().with_drag_and_drop(true);
```

### Handling `DragCompleted`

When the user drops, the widget emits
`ItemTreeEvent::DragCompleted { sources, target, position }`.
The `position` is a `DropPosition`:

| `DropPosition` | Effect on the model                              |
| -------------- | ------------------------------------------------ |
| `Before`       | Insert sources as siblings just before `target`  |
| `Into`         | Append sources as the last children of `target`  |
| `After`        | Insert sources as siblings just after `target`   |

The widget mutates nothing — your app applies the move, rebuilds
its `ItemNode<T>` tree, and calls `set_tree`. Key-based diffing
(RFC 001 §[D4]) then preserves expansion and selection for all
surviving ids — *including the moved nodes*, since identity is
orthogonal to position.

```rust,ignore
match msg {
    Message::Tree(ev) => {
        if let ItemTreeEvent::DragCompleted { sources, target, position } = &ev {
            apply_move(&mut self.model, sources.clone(), *target, *position);
            self.tree.set_tree_and_recompute_search(self.model.rebuild());
        }
        self.tree.update(ev).map(Message::Tree)
    }
    // ...
}
```

> **Important:** forward the widget's `Task` with `.map(Message::Tree)`.
> The deferred `Selected` (a click) and `DragCompleted` (a drop)
> are delivered as `Task::done` results — if you discard the Task
> (as the v0.8 example did), those events are never processed.

### Read-only drag accessors

```rust,ignore
tree.is_dragging();         // bool
tree.drag_sources();        // &[NodeId]
tree.drop_target();         // Option<(NodeId, DropPosition)>
```

See [`examples/item_tree.rs`](../../examples/item_tree.rs) for a
complete worked example that performs reorder/nest on drop.
