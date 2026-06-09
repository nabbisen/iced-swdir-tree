//! `ItemTree<T>` demo — a section outline for a Markdown document.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example item_tree
//! ```
//!
//! Demonstrates:
//!
//! * Building an `ItemTree<String>` from a fixed section outline.
//! * Keyboard navigation and multi-select.
//! * Live search (`set_tree_and_recompute_search` keeps search
//!   visible even after a simulated document re-parse).
//! * Re-parsing on a button click: node IDs that survive keep
//!   their expansion/selection state.

use std::fmt;

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length, Task};
use iced_swdir_tree::{ItemNode, ItemTree, ItemTreeEvent, NodeId};

// ---- domain type ----

#[derive(Debug, Clone)]
struct Section {
    level: u8,
    title: String,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = "  ".repeat((self.level - 1) as usize);
        write!(f, "{}H{} {}", prefix, self.level, self.title)
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

// ---- app ----

#[derive(Debug, Clone)]
enum Message {
    Tree(ItemTreeEvent),
    SearchChanged(String),
    Reparse,
}

struct App {
    tree: ItemTree<Section>,
    query: String,
    revision: u32,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut tree: ItemTree<Section> = ItemTree::new();
        tree.set_tree_and_recompute_search(build_outline(1));
        // Pre-expand the root so chapters are visible.
        let _ = tree.update(ItemTreeEvent::Toggled(NodeId(0)));
        let app = App {
            tree,
            query: String::new(),
            revision: 1,
        };
        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tree(ev) => {
                let _ = self.tree.update(ev);
                Task::none()
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
                self.tree
                    .set_tree_and_recompute_search(build_outline(self.revision));
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
                "No selection — click or use arrow keys.".into()
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
