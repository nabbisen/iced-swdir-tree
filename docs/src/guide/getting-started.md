# Getting started

## Install

```toml
[dependencies]
iced = "0.14"
iced-swdir-tree = "0.9"
```

The crate exposes two widgets. Pick the one that matches your data:

- Showing a **real directory** on disk? Use
  [`DirectoryTree`](directory-tree.md).
- Showing your **own in-memory tree** (an outline, a category tree, a
  scene graph)? Use [`ItemTree<T>`](item-tree.md).

Both share the same selection, keyboard, search, and drag-and-drop
model, so what you learn on one carries over.

## A directory tree in 30 seconds

```rust,no_run
use std::path::PathBuf;
use iced::{Element, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
}

struct App {
    tree: DirectoryTree,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let tree = DirectoryTree::new(PathBuf::from("."))
            .with_filter(DirectoryFilter::FilesAndFolders);
        (Self { tree }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tree(event) => self.tree.update(event).map(Message::Tree),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.tree.view(Message::Tree)
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view).run()
}
```

The key line is `self.tree.update(event).map(Message::Tree)`: the
widget returns a `Task` (for example, to run a directory scan off the
UI thread), and you must map and return it so those results flow back
in. Dropping the `Task` will make expansion appear to do nothing.

## An item tree in 30 seconds

```rust,no_run
use iced::{Element, Task};
use iced_swdir_tree::{ItemNode, ItemTree, ItemTreeEvent, NodeId};

#[derive(Debug, Clone)]
enum Message {
    Tree(ItemTreeEvent),
}

struct App {
    tree: ItemTree<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut tree = ItemTree::new();
        tree.set_tree(ItemNode {
            id: NodeId(0),
            data: "root".into(),
            children: vec![
                ItemNode { id: NodeId(1), data: "one".into(), children: vec![] },
                ItemNode { id: NodeId(2), data: "two".into(), children: vec![] },
            ],
        });
        (Self { tree }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tree(event) => self.tree.update(event).map(Message::Tree),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.tree.view(Message::Tree)
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view).run()
}
```

## Next steps

- Enable keyboard control — [Keyboard navigation](keyboard-navigation.md).
- Let users select several rows — [Multi-select](multi-select.md).
- Add a search box — [Incremental search](incremental-search.md).
- Reorder or nest by dragging — [Drag and drop](drag-and-drop.md).
- Swap in real icons — [Icon themes](icon-themes.md).
- Every knob in one place — [Configuration](configuration.md).

Working example apps live in the
[`examples/`](../../../examples/) directory. Run any with
`cargo run --example <name>` (for example, `item_tree`, `drag_drop`,
`search`).
