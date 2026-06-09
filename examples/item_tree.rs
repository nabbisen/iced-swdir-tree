//! `ItemTree<T>` demo — a section outline for a Markdown document,
//! with drag-and-drop reordering (RFC 002).
//!
//! Run with:
//!
//! ```sh
//! cargo run --example item_tree
//! ```
//!
//! Demonstrates:
//!
//! * Building an `ItemTree<Section>` from a fixed section outline.
//! * Keyboard navigation and multi-select.
//! * Live search (`set_tree_and_recompute_search` keeps search
//!   visible even after a simulated document re-parse).
//! * Re-parsing on a button click: node IDs that survive keep
//!   their expansion/selection state.
//! * **Drag-and-drop**: drag a section onto the thin strip above or
//!   below another to reorder it; drop it *onto* a section to nest
//!   it. The app owns the model — it applies the move and calls
//!   `set_tree`, and key-based diffing preserves expansion/selection
//!   across the edit.

use std::collections::HashSet;
use std::fmt;

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length, Task};
use iced_swdir_tree::{DropPosition, ItemNode, ItemTree, ItemTreeEvent, NodeId};

// ---- domain type ----

#[derive(Debug, Clone)]
struct Section {
    level: u8,
    title: String,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The tree view indents by depth already, so we only tag the
        // heading level here rather than adding our own indentation.
        write!(f, "H{} {}", self.level, self.title)
    }
}

// ---- document "parsing" ----

fn build_outline(v: u32) -> ItemNode<Section> {
    // Simulate a document with two H1 sections, each with H2 subsections.
    // The `v` parameter bumps one title so the UI shows the update
    // while preserving expansion state.
    let intro = ItemNode {
        id: NodeId(1),
        data: Section {
            level: 1,
            title: "Introduction".into(),
        },
        children: vec![
            ItemNode {
                id: NodeId(11),
                data: Section {
                    level: 2,
                    title: "Background".into(),
                },
                children: vec![],
            },
            ItemNode {
                id: NodeId(12),
                data: Section {
                    level: 2,
                    title: "Motivation".into(),
                },
                children: vec![],
            },
        ],
    };
    let design = ItemNode {
        id: NodeId(2),
        data: Section {
            level: 1,
            title: format!("Design (rev {v})"),
        },
        children: vec![
            ItemNode {
                id: NodeId(21),
                data: Section {
                    level: 2,
                    title: "Architecture".into(),
                },
                children: vec![ItemNode {
                    id: NodeId(211),
                    data: Section {
                        level: 3,
                        title: "Core types".into(),
                    },
                    children: vec![],
                }],
            },
            ItemNode {
                id: NodeId(22),
                data: Section {
                    level: 2,
                    title: "Data model".into(),
                },
                children: vec![],
            },
        ],
    };
    ItemNode {
        id: NodeId(0),
        data: Section {
            level: 1,
            title: "Document".into(),
        },
        children: vec![intro, design],
    }
}

// ---- tree surgery: applying a DragCompleted to the model ----

/// Remove every node whose id is in `set` from `node`'s subtree,
/// collecting the removed subtrees (an ancestor in `set` takes its
/// descendants with it, so they are not collected separately).
fn extract(
    node: &mut ItemNode<Section>,
    set: &HashSet<NodeId>,
    removed: &mut Vec<ItemNode<Section>>,
) {
    let mut kept = Vec::new();
    for mut child in std::mem::take(&mut node.children) {
        if set.contains(&child.id) {
            removed.push(child);
        } else {
            extract(&mut child, set, removed);
            kept.push(child);
        }
    }
    node.children = kept;
}

/// Append `items` as the last children of the node with id `target`.
/// Returns the items back (un-inserted) if `target` wasn't found.
fn insert_into(
    node: &mut ItemNode<Section>,
    target: NodeId,
    items: Vec<ItemNode<Section>>,
) -> Option<Vec<ItemNode<Section>>> {
    if node.id == target {
        node.children.extend(items);
        return None;
    }
    let mut carry = Some(items);
    for child in &mut node.children {
        match insert_into(child, target, carry.take().unwrap()) {
            None => return None,
            Some(back) => carry = Some(back),
        }
    }
    carry
}

/// Insert `items` as siblings just before (or after) `target`.
fn insert_sibling(
    node: &mut ItemNode<Section>,
    target: NodeId,
    items: Vec<ItemNode<Section>>,
    after: bool,
) -> Option<Vec<ItemNode<Section>>> {
    if let Some(pos) = node.children.iter().position(|c| c.id == target) {
        let at = if after { pos + 1 } else { pos };
        let tail = node.children.split_off(at);
        node.children.extend(items);
        node.children.extend(tail);
        return None;
    }
    let mut carry = Some(items);
    for child in &mut node.children {
        match insert_sibling(child, target, carry.take().unwrap(), after) {
            None => return None,
            Some(back) => carry = Some(back),
        }
    }
    carry
}

fn apply_move(
    outline: &mut ItemNode<Section>,
    sources: Vec<NodeId>,
    target: NodeId,
    position: DropPosition,
) {
    let set: HashSet<NodeId> = sources.into_iter().collect();
    let mut removed = Vec::new();
    extract(outline, &set, &mut removed);
    if removed.is_empty() {
        return;
    }
    let _ = match position {
        DropPosition::Into => insert_into(outline, target, removed),
        DropPosition::Before => insert_sibling(outline, target, removed, false),
        DropPosition::After => insert_sibling(outline, target, removed, true),
    };
}

// ---- app ----

#[derive(Debug, Clone)]
enum Message {
    Tree(ItemTreeEvent),
    SearchChanged(String),
    Reparse,
}

struct App {
    tree: ItemTree<Section>,
    outline: ItemNode<Section>,
    query: String,
    revision: u32,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let outline = build_outline(1);
        let mut tree: ItemTree<Section> = ItemTree::new().with_drag_and_drop(true);
        tree.set_tree_and_recompute_search(outline.clone());
        // Pre-expand the root so chapters are visible.
        let _ = tree.update(ItemTreeEvent::Toggled(NodeId(0)));
        let app = App {
            tree,
            outline,
            query: String::new(),
            revision: 1,
        };
        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tree(ev) => {
                // Observe a completed drag and mutate our own model,
                // then re-sync the widget. Diffing preserves the
                // moved nodes' expansion/selection state.
                if let ItemTreeEvent::DragCompleted {
                    sources,
                    target,
                    position,
                } = &ev
                {
                    apply_move(&mut self.outline, sources.clone(), *target, *position);
                    self.tree
                        .set_tree_and_recompute_search(self.outline.clone());
                }
                // Forward to the widget and propagate its Task — this
                // is what delivers the deferred Selected (a click) and
                // the DragCompleted (a drop) back to us.
                self.tree.update(ev).map(Message::Tree)
            }
            Message::SearchChanged(q) => {
                self.query = q.clone();
                self.tree.set_search_query(q);
                Task::none()
            }
            Message::Reparse => {
                self.revision += 1;
                // Simulate a document re-parse. set_tree_and_recompute_search
                // preserves expansion/selection state for stable NodeIds.
                self.outline = build_outline(self.revision);
                self.tree
                    .set_tree_and_recompute_search(self.outline.clone());
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let search_bar = text_input("Search sections…", &self.query)
            .on_input(Message::SearchChanged)
            .padding(6);

        let reparse_btn = button("Re-parse document").on_press(Message::Reparse);

        let status = if self.tree.is_searching() {
            format!(
                "{} match{} for \"{}\"",
                self.tree.search_match_count(),
                if self.tree.search_match_count() == 1 {
                    ""
                } else {
                    "es"
                },
                self.query
            )
        } else {
            let n = self.tree.selected_ids().len();
            if n == 0 {
                "Drag a row to reorder; drop onto a row to nest. Click or arrow-key to select."
                    .into()
            } else {
                format!("{n} section{} selected.", if n == 1 { "" } else { "s" })
            }
        };

        column![
            row![search_bar, reparse_btn].spacing(8),
            container(self.tree.view(Message::Tree))
                .width(Length::Fill)
                .height(Length::Fill),
            text(status).size(12),
        ]
        .spacing(8)
        .padding(10)
        .into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("iced-swdir-tree · ItemTree example")
        .run()
}
