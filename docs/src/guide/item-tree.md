# Generic item tree

`ItemTree<T>` is a synchronous, in-memory tree over your own data type
`T`. Unlike `DirectoryTree` it never touches disk and has no async
scan lifecycle — you hand it a tree, it renders and navigates it.

It is the right choice for outlines, category browsers, scene graphs,
settings hierarchies, or any tree whose contents you already hold in
memory.

## Nodes and identity

You describe the tree with `ItemNode<T>`:

```rust,ignore
use iced_swdir_tree::{ItemNode, NodeId};

let root = ItemNode {
    id: NodeId(0),
    data: my_section,          // any T: Clone + Debug + Send + Sync + 'static
    children: vec![/* ItemNode<T> … */],
};
```

Every node carries a `NodeId(u64)` that **you** assign. These ids are
the tree's identity: expansion state, selection, and drag targets are
all tracked by `NodeId`, independent of a node's position. Keep an
id stable across edits and the widget keeps that node's state across
edits.

For the built-in view, `T` also needs to be `Display` — that is how a
row renders its label.

## Feeding and updating the tree

```rust,ignore
let mut tree = ItemTree::new();
tree.set_tree(root);
```

Call `set_tree` again whenever your data changes. It performs
**key-based diffing**: for every `NodeId` that still exists, expansion
and selection are preserved; ids that disappeared are dropped; new ids
start collapsed and unselected. This means you can rebuild the whole
tree on every edit without losing UI state — no manual reconciliation.

If a live search is active, use `set_tree_and_recompute_search` so the
filtered view stays consistent after the rebuild.

## Reacting to events

Thread `ItemTreeEvent` through `update` and return the `Task`:

```rust,ignore
Message::Tree(event) => self.tree.update(event).map(Message::Tree),
```

`Selected(id, mode)` tells you the selection changed. If you enable
drag-and-drop, `DragCompleted { sources, target, position }` tells you
the user wants to move `sources` relative to `target`. As always, the
widget mutates nothing — you apply the move to your model and call
`set_tree`. See [Drag and drop](drag-and-drop.md) for the reorder/nest
model and a worked example.

## What carries over from DirectoryTree

Selection modes, keyboard navigation, and incremental search behave
identically to `DirectoryTree` — the same pages apply:
[Multi-select](multi-select.md),
[Keyboard navigation](keyboard-navigation.md),
[Incremental search](incremental-search.md).

The `item_tree` example is a complete outline editor with search,
re-parse, and drag-and-drop reordering — run it with
`cargo run --example item_tree`.
