//! Multi-select demo — Shift/Ctrl-click and multi-row keyboard ranges.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example multi_select -- /path/to/browse
//! ```
//!
//! Defaults to the current directory.
//!
//! # The pattern
//!
//! iced 0.14's `button::on_press` callback cannot observe modifier
//! keys. The built-in widget view therefore always emits
//! `DirectoryTreeEvent::Selected(path, is_dir, SelectionMode::Replace)`
//! — i.e. it treats every click like a plain click.
//!
//! For Shift-click and Ctrl-click to actually do multi-select, the
//! *application* tracks modifier state separately (via the keyboard
//! subscription), intercepts `Selected` events in its own update
//! handler, and rewrites the mode using
//! [`SelectionMode::from_modifiers`] before forwarding to
//! `tree.update`.
//!
//! This mirrors how most iced apps handle modifier-aware input and
//! keeps the widget focus-neutral.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::keyboard::{self, Modifiers};
use iced::widget::{Column, column, container, scrollable, text};
use iced::{Element, Length, Subscription, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
    ModifiersChanged(Modifiers),
    Key(keyboard::Key, Modifiers),
}

struct App {
    tree: DirectoryTree,
    /// Most recent modifier state observed. Used to rewrite
    /// incoming `Selected` clicks into the appropriate mode.
    modifiers: Modifiers,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let root = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let root_for_task = root.clone();
        let tree = DirectoryTree::new(root).with_filter(DirectoryFilter::FilesAndFolders);
        (
            Self {
                tree,
                modifiers: Modifiers::default(),
            },
            // Kick off the first expansion so the user sees content.
            Task::done(Message::Tree(DirectoryTreeEvent::Toggled(root_for_task))),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Intercept plain-click `Selected` events and rewrite the
            // mode based on the current modifier state. The built-in
            // view always produces Replace; keyboard events produce
            // the right mode already (handled by handle_key).
            Message::Tree(DirectoryTreeEvent::Selected(path, is_dir, _from_view)) => {
                let mode = SelectionMode::from_modifiers(self.modifiers);
                let event = DirectoryTreeEvent::Selected(path, is_dir, mode);
                self.tree.update(event).map(Message::Tree)
            }
            Message::Tree(event) => self.tree.update(event).map(Message::Tree),
            Message::ModifiersChanged(m) => {
                self.modifiers = m;
                Task::none()
            }
            Message::Key(key, mods) => {
                if let Some(event) = self.tree.handle_key(&key, mods) {
                    return self.tree.update(event).map(Message::Tree);
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // `keyboard::listen()` surfaces both key events AND modifier
        // change events. We route both: key presses into the tree's
        // `handle_key`, modifier changes into our own tracking.
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => Message::Key(key, modifiers),
            keyboard::Event::ModifiersChanged(modifiers) => Message::ModifiersChanged(modifiers),
            // Non-press/non-modifier events: route as a harmless key
            // that handle_key leaves unbound.
            _ => Message::Key(
                keyboard::Key::Named(keyboard::key::Named::F35),
                Modifiers::default(),
            ),
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let selected = self.tree.selected_paths();
        let count = selected.len();

        // Human-readable summary of currently-selected rows.
        let summary_text = if count == 0 {
            "No selection. Click to select, Shift+click for range, \
             Ctrl/Cmd+click to toggle."
                .to_string()
        } else {
            format!(
                "{count} selected (anchor: {})",
                self.tree
                    .anchor_path()
                    .map(short_name)
                    .unwrap_or_else(|| "-".into())
            )
        };

        // Compact list of selected basenames. Capped to avoid the
        // status bar eating the screen when the user ranges over
        // a huge folder.
        const MAX_SHOWN: usize = 10;
        let names: HashSet<String> = selected.iter().map(|p| short_name(p)).collect();
        let mut names_sorted: Vec<String> = names.into_iter().collect();
        names_sorted.sort();
        let shown: String = if names_sorted.len() <= MAX_SHOWN {
            names_sorted.join(", ")
        } else {
            format!(
                "{}, +{} more",
                names_sorted
                    .iter()
                    .take(MAX_SHOWN)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                names_sorted.len() - MAX_SHOWN
            )
        };

        let status = Column::new()
            .push(text(summary_text).size(13))
            .push(text(shown).size(11))
            .spacing(2);

        container(
            column![
                scrollable(self.tree.view(Message::Tree)).height(Length::Fill),
                status,
            ]
            .spacing(8.0)
            .padding(8.0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn short_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("iced-swdir-tree · multi-select example")
        .run()
}
