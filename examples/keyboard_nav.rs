//! Keyboard-driven directory browsing.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example keyboard_nav -- /path/to/browse
//! ```
//!
//! Defaults to the current directory. Navigate with the arrow keys,
//! `Enter` to expand/collapse folders, `Space` to re-emit the current
//! selection, `Home`/`End` to jump to the first/last visible row.
//!
//! The pattern this demonstrates:
//!
//! 1. Subscribe to `iced::keyboard::on_key_press` in `subscription()`.
//! 2. Pipe each key through `DirectoryTree::handle_key`.
//! 3. If a synthetic [`DirectoryTreeEvent`] comes back, route it to
//!    `DirectoryTree::update` like any other event.

use std::path::PathBuf;

use iced::keyboard::{self, Modifiers};
use iced::widget::{column, container, text};
use iced::{Element, Length, Subscription, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
    Key(keyboard::Key, Modifiers),
}

struct App {
    tree: DirectoryTree,
    last_selected: Option<PathBuf>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let root = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        // Proactively expand the root so the user sees something
        // without having to click or key-press first.
        let root_for_task = root.clone();
        let tree = DirectoryTree::new(root).with_filter(DirectoryFilter::FilesAndFolders);
        (
            Self {
                tree,
                last_selected: None,
            },
            Task::done(Message::Tree(DirectoryTreeEvent::Toggled(root_for_task))),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tree(event) => {
                if let DirectoryTreeEvent::Selected(p, _, _) = &event {
                    self.last_selected = Some(p.clone());
                }
                self.tree.update(event).map(Message::Tree)
            }
            Message::Key(key, mods) => {
                // handle_key is `&self` — it only *produces* an
                // event. We still have to route the returned event
                // back through update so state transitions (cursor
                // move, expand/collapse) actually happen.
                if let Some(event) = self.tree.handle_key(&key, mods) {
                    if let DirectoryTreeEvent::Selected(p, _, _) = &event {
                        self.last_selected = Some(p.clone());
                    }
                    return self.tree.update(event).map(Message::Tree);
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // In a real app you'd gate this on "does the tree have
        // focus?" — a checkbox in settings, a sidebar toggle,
        // etc. Here the tree is the whole UI so it always has focus.
        //
        // `iced::keyboard::listen()` exposes every keyboard event as
        // a `keyboard::Event`. We want key presses only; non-press
        // events (KeyReleased, ModifiersChanged) are handled by the
        // widget with `handle_key` returning `None`, so we can
        // cheaply forward the non-KeyPressed placeholder values too
        // — but it's tidier to map them into a no-op `Message::Tree`
        // that gets dropped by `update`'s match.
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => Message::Key(key, modifiers),
            // Non-press events: use a dummy key that handle_key
            // leaves unbound so update() returns Task::none().
            _ => Message::Key(
                keyboard::Key::Named(keyboard::key::Named::F35),
                Modifiers::default(),
            ),
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let status = text(match &self.last_selected {
            Some(p) => format!(
                "Selected: {}  |  Try ↑ ↓ ← →, Enter, Space, Home, End.",
                p.display()
            ),
            None => "Press ↓ to select the first row.".into(),
        })
        .size(12);

        container(
            column![self.tree.view(Message::Tree), status]
                .spacing(8.0)
                .padding(8.0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("iced-swdir-tree · keyboard navigation example")
        .run()
}
