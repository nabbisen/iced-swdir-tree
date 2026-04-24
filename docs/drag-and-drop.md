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

See [`examples/drag_drop.rs`](../examples/drag_drop.rs) for a
complete working app with `fs::rename` on drop, post-move
refresh, and a live drag-preview status bar.
