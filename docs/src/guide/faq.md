# FAQ

### Which widget do I want — `DirectoryTree` or `ItemTree<T>`?

If the tree *is* a real directory on disk, use `DirectoryTree`: it
scans lazily and asynchronously for you. If you already hold the tree
in memory (an outline, a category list, anything over your own type),
use `ItemTree<T>`. See [Getting started](getting-started.md).

### Why does nothing happen when I expand a folder?

You are almost certainly dropping the `Task` that `update` returns.
Expansion dispatches an asynchronous scan, and its result comes back
through that `Task`. Always write
`self.tree.update(event).map(Message::Tree)` and return it.

### The tree does not respond to the keyboard. Why?

Keyboard handling is opt-in: you subscribe to key events and forward
them through `handle_key`. See
[Keyboard navigation](keyboard-navigation.md) for the wiring.

### I dropped an item but nothing moved.

That is by design. The widget reports drag-and-drop as an event
(`DragCompleted`) and performs no change itself — it never writes to
disk or mutates your model. Your app handles the event and performs
the move. See [Drag and drop](drag-and-drop.md).

### Does drag-and-drop work on `ItemTree`?

Yes, as of v0.9.0, and it is **opt-in**: call
`ItemTree::with_drag_and_drop(true)`. It supports both reordering
(drop between siblings) and nesting (drop onto a node). See
[Drag and drop](drag-and-drop.md).

### I rebuilt my `ItemTree` and lost which rows were expanded.

Make sure the `NodeId`s are stable across rebuilds. `set_tree`
preserves expansion and selection for every id that still exists; if
your ids change on each rebuild, the widget cannot recognise the
nodes. See [Generic item tree](item-tree.md).

### Do I need the `icons` feature?

Only if you want the bundled lucide glyph font. Without it the widget
uses Unicode-symbol fallbacks, and you can still plug in any icon set
by implementing `IconTheme`. See [Icon themes](icon-themes.md).

### Will the widget delete or move my files?

No. Neither widget ever writes, renames, deletes, or moves anything.
`DirectoryTree` only reads directory listings; `ItemTree` only reads
the data you give it. All mutation stays in your application.

### Where are the runnable examples?

In the [`examples/`](../../../examples/) directory. Run any with
`cargo run --example <name>` — for instance `item_tree`, `drag_drop`,
`multi_select`, `search`, `keyboard_nav`, or `icon_theme`.
