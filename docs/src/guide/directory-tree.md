# Directory tree

`DirectoryTree` renders a real filesystem directory. It loads lazily
(only the root is created up front; each folder is scanned when it is
first expanded), runs the scan off the UI thread, and caches results so
that filtering and search never re-scan.

## Creating one

```rust,ignore
use std::path::PathBuf;
use iced_swdir_tree::{DirectoryFilter, DirectoryTree};

let tree = DirectoryTree::new(PathBuf::from("/some/project"))
    .with_filter(DirectoryFilter::FilesAndFolders);
```

`DirectoryFilter` decides what appears: files and folders, folders
only, or a custom rule. See [Configuration](configuration.md) for the
full set of builder options (`max_depth`, prefetch, icon theme, custom
executor).

## Wiring it up

A `DirectoryTree` is driven the same way as any iced component — thread
its event through `update` and return the resulting `Task`:

```rust,ignore
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::Tree(event) => self.tree.update(event).map(Message::Tree),
    }
}

fn view(&self) -> Element<'_, Message> {
    self.tree.view(Message::Tree)
}
```

Returning the mapped `Task` is essential: expanding a folder dispatches
an asynchronous scan, and its result is delivered as a follow-up event
through that `Task`.

## Reacting to what the user does

Your `Message::Tree(DirectoryTreeEvent)` arm receives everything the
user does. The two events you will usually act on yourself are
selection and drag-and-drop:

- `Selected(path, is_dir, mode)` — the active selection changed.
- `DragCompleted { sources, destination }` — the user dropped
  `sources` onto the `destination` folder. **The widget performs no
  filesystem operation** — you perform the move/copy/upload and then
  refresh the affected subtree.

The remaining variants (`Toggled`, `Drag`, `Loaded`) are internal
plumbing; forward them to `update` and otherwise ignore them.

See the full list in [Events](../reference/events.md), and the
drag-and-drop responsibility split in [Drag and drop](drag-and-drop.md).

## What it will not do

`DirectoryTree` never writes to disk. It reads directory listings and
nothing else — no rename, delete, move, or create. That keeps the
widget safe to drop into any app: the destructive decisions stay in
your hands.
