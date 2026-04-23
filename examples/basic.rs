//! A minimal iced application using [`DirectoryTree`] with the text
//! icon fallback (no `icons` feature required).
//!
//! Run with:
//!
//! ```sh
//! cargo run --example basic -- /path/to/browse
//! ```
//!
//! Defaults to the current working directory if no path is given.

use std::path::PathBuf;

use iced::{Element, Length, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
    SetFilter(DirectoryFilter),
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
        let tree = DirectoryTree::new(root).with_filter(DirectoryFilter::FilesAndFolders);
        (
            Self {
                tree,
                last_selected: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tree(event) => {
                // Side-effect: remember the last selection so we can
                // show it in the status bar.
                if let DirectoryTreeEvent::Selected(p, _) = &event {
                    self.last_selected = Some(p.clone());
                }
                self.tree.update(event).map(Message::Tree)
            }
            Message::SetFilter(filter) => {
                self.tree.set_filter(filter);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, text};

        // Three plain buttons for filter selection. Keeps the example
        // dependency-free of `Display` impls or pick_list complexity.
        let filter_row = row![
            text("Filter:"),
            button("Folders only")
                .on_press(Message::SetFilter(DirectoryFilter::FoldersOnly))
                .style(if self.tree.filter() == DirectoryFilter::FoldersOnly {
                    button::primary
                } else {
                    button::secondary
                }),
            button("Files + folders")
                .on_press(Message::SetFilter(DirectoryFilter::FilesAndFolders))
                .style(if self.tree.filter() == DirectoryFilter::FilesAndFolders {
                    button::primary
                } else {
                    button::secondary
                }),
            button("All (w/ hidden)")
                .on_press(Message::SetFilter(DirectoryFilter::AllIncludingHidden))
                .style(
                    if self.tree.filter() == DirectoryFilter::AllIncludingHidden {
                        button::primary
                    } else {
                        button::secondary
                    },
                ),
        ]
        .spacing(8.0);

        let status = text(match &self.last_selected {
            Some(p) => format!("Selected: {}", p.display()),
            None => "No selection yet. Click any row to select; click folders to expand.".into(),
        });

        container(
            column![filter_row, self.tree.view(Message::Tree), status]
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
        .title("iced-swdir-tree · basic example")
        .run()
}
