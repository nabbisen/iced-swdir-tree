//! The same functionality as [`basic`], but with real lucide-icons
//! glyphs via the `icons` feature.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example with_icons --features icons -- /path/to/browse
//! ```
//!
//! The only difference from `basic` is the `iced::application` call:
//! we have to register the bundled lucide TTF with iced, otherwise the
//! glyphs render as tofu squares. The crate re-exports
//! [`LUCIDE_FONT_BYTES`] for exactly this purpose.

use std::path::PathBuf;

use iced::{Element, Length, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, LUCIDE_FONT_BYTES};

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
                if let DirectoryTreeEvent::Selected(p, _, _) = &event {
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
    // Register the lucide TTF *before* rendering, otherwise the icon
    // widget emits text that the default font can't draw. This is the
    // one extra step compared to the `basic` example.
    iced::application(App::new, App::update, App::view)
        .title("iced-swdir-tree · with-icons example")
        .font(LUCIDE_FONT_BYTES)
        .run()
}
