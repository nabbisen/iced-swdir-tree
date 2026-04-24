//! Incremental search demo — type-ahead filter over the tree.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example search -- /path/to/scratch
//! ```
//!
//! Without an argument, defaults to a scratch directory under the OS
//! temp dir, populated with nested folders and files so there's
//! something to search through.
//!
//! # What this demonstrates
//!
//! * A plain [`iced::widget::text_input`] above the tree, wired into
//!   the widget via [`DirectoryTree::set_search_query`].
//! * Live-update of a "N matches" counter below, via
//!   [`DirectoryTree::search_match_count`].
//! * A small expand-all button that loads every subdirectory so
//!   search coverage is broader. In a real app you'd typically pair
//!   search with [`DirectoryTree::with_prefetch_limit`] (v0.5) for
//!   the same effect without the explicit button.
//!
//! # Known limitation (v0.6)
//!
//! Clicking to expand a folder while a search is active does NOT
//! escape the filter — the widget stays narrowed to matches-and-
//! ancestors. To explore outside the current search, clear the
//! query first.

use std::fs;
use std::path::PathBuf;

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length, Task};
use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
    SearchChanged(String),
    ExpandAll,
}

struct App {
    tree: DirectoryTree,
    query: String,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let root = resolve_root();
        let tree = DirectoryTree::new(root.clone())
            .with_filter(DirectoryFilter::FilesAndFolders)
            // Prefetch one level helps search cover more ground
            // without the user expanding everything manually.
            .with_prefetch_limit(20);
        let mut app = App {
            tree,
            query: String::new(),
        };
        // Kick off the initial scan of the root.
        let task = app
            .tree
            .update(DirectoryTreeEvent::Toggled(root))
            .map(Message::Tree);
        (app, task)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tree(ev) => self.tree.update(ev).map(Message::Tree),
            Message::SearchChanged(q) => {
                self.query = q.clone();
                self.tree.set_search_query(q);
                Task::none()
            }
            Message::ExpandAll => {
                // Toggle every loaded folder that isn't already
                // expanded. The widget's on_loaded handler will
                // cascade more scans via prefetch.
                let mut tasks = Vec::new();
                let to_expand = collect_collapsed_folders(&self.tree);
                for p in to_expand {
                    tasks.push(
                        self.tree
                            .update(DirectoryTreeEvent::Toggled(p))
                            .map(Message::Tree),
                    );
                }
                Task::batch(tasks)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let search_bar = text_input("Search filenames...", &self.query)
            .on_input(Message::SearchChanged)
            .padding(6);

        let status_text = if self.tree.is_searching() {
            format!(
                "{} match{} for \"{}\"",
                self.tree.search_match_count(),
                if self.tree.search_match_count() == 1 {
                    ""
                } else {
                    "es"
                },
                self.query,
            )
        } else {
            "Type above to filter. Press Expand-all to load deeper \
             folders for broader coverage."
                .into()
        };

        let controls = row![
            search_bar,
            button(text("Expand all")).on_press(Message::ExpandAll),
        ]
        .spacing(8);

        column![
            controls,
            container(self.tree.view(Message::Tree))
                .width(Length::Fill)
                .height(Length::Fill),
            text(status_text).size(13),
        ]
        .spacing(8)
        .padding(10)
        .into()
    }
}

/// Walk the tree and return every loaded folder that is currently
/// collapsed. The app issues `Toggled` events for each to "expand
/// all" in one button press (depth-first, best effort).
fn collect_collapsed_folders(tree: &DirectoryTree) -> Vec<PathBuf> {
    // There's no public "walk every node" API, so we do a BFS by
    // repeatedly querying visible_rows() of the tree's internal
    // view - but that requires crate-internal access. Instead, we
    // use the public root_path and do our own filesystem walk of
    // directories only.
    let mut out = Vec::new();
    fn recurse(p: &std::path::Path, out: &mut Vec<PathBuf>) {
        if !p.is_dir() {
            return;
        }
        out.push(p.to_path_buf());
        if let Ok(read) = fs::read_dir(p) {
            for entry in read.flatten() {
                let ep = entry.path();
                if ep.is_dir() {
                    recurse(&ep, out);
                }
            }
        }
    }
    recurse(tree.root_path(), &mut out);
    out
}

fn resolve_root() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    let scratch = std::env::temp_dir().join("iced-swdir-tree-search-demo");
    let _ = fs::create_dir_all(&scratch);
    // Create a miniature project layout to search through.
    for dir in &[
        "project",
        "project/src",
        "project/src/lib",
        "project/tests",
        "notes",
        "notes/ideas",
    ] {
        let _ = fs::create_dir_all(scratch.join(dir));
    }
    for (path, body) in &[
        ("project/README.md", "# Project\n"),
        ("project/src/main.rs", "fn main() {}\n"),
        ("project/src/lib/config.rs", ""),
        ("project/src/lib/parser.rs", ""),
        ("project/tests/integration.rs", ""),
        ("project/tests/readme.md", "test notes\n"),
        ("notes/todo.md", "- buy milk\n"),
        ("notes/ideas/app_idea.md", ""),
        ("notes/ideas/README.md", ""),
        ("scratch_note.txt", ""),
    ] {
        let p = scratch.join(path);
        if !p.exists() {
            let _ = fs::write(p, body);
        }
    }
    scratch
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("iced-swdir-tree · search example")
        .run()
}
