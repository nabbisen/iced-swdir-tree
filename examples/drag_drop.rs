//! Drag-and-drop demo — move files and folders between nodes.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example drag_drop -- /path/to/scratch
//! ```
//!
//! Defaults to a *freshly created* scratch directory under the OS
//! temp dir so you can't lose real data while experimenting. The
//! scratch dir is populated with a few folders and files on first
//! run so there's something to drag around.
//!
//! # What the example demonstrates
//!
//! The `DirectoryTree` widget itself performs **no** filesystem
//! work: it emits a `DirectoryTreeEvent::DragCompleted { sources,
//! destination }` event when the user drops one or more paths onto
//! a valid folder, and the application decides what to actually do
//! with them. This example handles the event by calling
//! [`std::fs::rename`] on each source, then refreshes both the
//! source-parent folders and the destination folder so the widget
//! picks up the new layout on screen.
//!
//! # Multi-item drag and Esc-to-cancel
//!
//! Exactly as in `examples/multi_select.rs`, modifier tracking is
//! done at the application layer so that Shift/Ctrl-click build up
//! a multi-selection. Pressing the mouse on an already-selected row
//! drags the whole set; pressing on a row outside the selection
//! drags just that row. `Escape` cancels an in-flight drag (the
//! widget's built-in binding — see `keyboard.rs`).

use std::collections::HashSet;
use std::fs;
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
    modifiers: Modifiers,
    /// Last status line to show under the tree. Either a
    /// drag-preview ("Drop N items onto X?") or the result of the
    /// most recent move operation ("Moved 3 items into X").
    status: String,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let root = resolve_root();
        let root_for_task = root.clone();
        let tree = DirectoryTree::new(root).with_filter(DirectoryFilter::FilesAndFolders);
        (
            Self {
                tree,
                modifiers: Modifiers::default(),
                status: "Drag a row onto a folder to move it. \
                         Shift/Ctrl-click for multi-select. \
                         Esc cancels an in-flight drag."
                    .to_string(),
            },
            Task::done(Message::Tree(DirectoryTreeEvent::Toggled(root_for_task))),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // As in the multi-select example, rewrite the built-in
            // view's `Replace`-only `Selected` events using the
            // current modifier state. Keyboard events come through
            // `handle_key` with the correct mode already.
            Message::Tree(DirectoryTreeEvent::Selected(path, is_dir, _)) => {
                let mode = SelectionMode::from_modifiers(self.modifiers);
                let event = DirectoryTreeEvent::Selected(path, is_dir, mode);
                self.tree.update(event).map(Message::Tree)
            }
            // The headline case: the user released the mouse over a
            // valid drop target. Perform the actual filesystem
            // operation, then refresh affected folders so the tree
            // view reflects the new layout.
            Message::Tree(DirectoryTreeEvent::DragCompleted {
                sources,
                destination,
            }) => {
                let outcome = move_paths(&sources, &destination);
                self.status = outcome.summary();
                // The set of folders that need re-scanning: the
                // destination (for the newly-arrived entries) and
                // every source's parent (for the departed entries).
                let mut to_refresh: HashSet<PathBuf> = HashSet::new();
                to_refresh.insert(destination);
                for s in &sources {
                    if let Some(parent) = s.parent() {
                        to_refresh.insert(parent.to_path_buf());
                    }
                }
                // Issue a collapse+expand for each affected folder.
                // A collapse followed by a `Toggled` on the same
                // path is the simplest way in v0.4 to invalidate
                // the cached children and re-scan from scratch.
                let tasks: Vec<Task<Message>> = to_refresh
                    .into_iter()
                    .flat_map(|p| {
                        [
                            Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p.clone()))),
                            Task::done(Message::Tree(DirectoryTreeEvent::Toggled(p))),
                        ]
                    })
                    .collect();
                Task::batch(tasks)
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
        keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => Message::Key(key, modifiers),
            keyboard::Event::ModifiersChanged(modifiers) => Message::ModifiersChanged(modifiers),
            _ => Message::Key(
                keyboard::Key::Named(keyboard::key::Named::F35),
                Modifiers::default(),
            ),
        })
    }

    fn view(&self) -> Element<'_, Message> {
        // While a drag is in progress, override the static status
        // line with a live preview of where the drop will land.
        let live_status = if self.tree.is_dragging() {
            match self.tree.drop_target() {
                Some(dest) => format!(
                    "Drop {} onto {}?",
                    describe_sources(self.tree.drag_sources()),
                    short_name(dest),
                ),
                None => format!(
                    "Dragging {} — hover over a folder",
                    describe_sources(self.tree.drag_sources()),
                ),
            }
        } else {
            self.status.clone()
        };

        let status = Column::new().push(text(live_status).size(13)).spacing(2);

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

/// Determine the root path to browse.
///
/// If the user passes a path on the command line, use that.
/// Otherwise, create (or reuse) a scratch directory under the OS
/// temp dir populated with some example files, so that this
/// example is always "safe" to run and produces something to drag
/// around.
fn resolve_root() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    let scratch = std::env::temp_dir().join("iced-swdir-tree-drag-demo");
    let _ = fs::create_dir_all(&scratch);
    // Populate once. If the user already has files in here we
    // leave them alone; the demo just has more to look at.
    for folder in &["inbox", "archive", "drafts"] {
        let _ = fs::create_dir_all(scratch.join(folder));
    }
    for (name, body) in &[
        ("notes.txt", "drop me somewhere"),
        ("todo.md", "- try dragging this into `inbox`\n"),
        ("ideas.txt", "multi-select me with Ctrl or Shift"),
    ] {
        let p = scratch.join(name);
        if !p.exists() {
            let _ = fs::write(p, body);
        }
    }
    scratch
}

/// Result of `move_paths`: how many moves succeeded and how many
/// failed. `dest` lets us compose a nice status message.
struct MoveOutcome {
    moved: usize,
    failed: Vec<(PathBuf, std::io::Error)>,
    dest: PathBuf,
}

impl MoveOutcome {
    fn summary(&self) -> String {
        match (self.moved, self.failed.len()) {
            (n, 0) => format!(
                "Moved {} item{} into {}",
                n,
                plural(n),
                short_name(&self.dest)
            ),
            (0, f) => format!(
                "Failed to move {} item{} into {}: {}",
                f,
                plural(f),
                short_name(&self.dest),
                self.failed[0].1,
            ),
            (n, f) => format!(
                "Moved {} into {}, {} failed (e.g. {}: {})",
                n,
                short_name(&self.dest),
                f,
                short_name(&self.failed[0].0),
                self.failed[0].1,
            ),
        }
    }
}

/// Move each path in `sources` into `dest`.
///
/// Uses `std::fs::rename` which is atomic within a single
/// filesystem. Real apps might want to fall back to copy+delete
/// across mount points, preserve mtimes, etc. — this example keeps
/// it short.
fn move_paths(sources: &[PathBuf], dest: &Path) -> MoveOutcome {
    let mut moved = 0;
    let mut failed = Vec::new();
    for src in sources {
        let Some(name) = src.file_name() else {
            continue;
        };
        let target = dest.join(name);
        // Guard against overwriting: if the target exists, skip.
        if target.exists() {
            failed.push((
                src.clone(),
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "destination already has an entry with that name",
                ),
            ));
            continue;
        }
        match fs::rename(src, &target) {
            Ok(()) => moved += 1,
            Err(e) => failed.push((src.clone(), e)),
        }
    }
    MoveOutcome {
        moved,
        failed,
        dest: dest.to_path_buf(),
    }
}

/// Describe the drag-sources slice for status-bar display.
/// "notes.txt" for 1 item, "3 items" for more.
fn describe_sources(sources: &[PathBuf]) -> String {
    match sources {
        [] => "nothing".into(),
        [p] => short_name(p),
        _ => format!("{} items", sources.len()),
    }
}

fn short_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("iced-swdir-tree · drag-and-drop example")
        .run()
}
