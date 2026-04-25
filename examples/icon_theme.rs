//! Custom icon-theme demo — a three-way theme switcher.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example icon_theme
//! ```
//!
//! A pick-list at the top cycles through three themes:
//!
//! * **Unicode** — the crate's default when the `icons` feature
//!   is off; renders emoji-like Unicode symbols (📁 📂 📄 ⚠ ▸ ▾).
//! * **Label** — a custom theme defined below that renders short
//!   text labels (`[DIR]`, `[FILE]`, …) in the default font.
//! * **Ascii** — another custom theme using single-character
//!   ASCII symbols so the row height stays tight.
//!
//! This demonstrates the `IconTheme` trait surface end-to-end:
//! how to implement a theme, how to swap between themes at
//! runtime (just rebuild the tree with a new
//! `Arc<dyn IconTheme>`), and how the crate's stock themes fit
//! into the same trait as your own.

use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length, Task};
use iced_swdir_tree::{
    DirectoryFilter, DirectoryTree, DirectoryTreeEvent, IconRole, IconSpec, IconTheme, UnicodeTheme,
};

/// Custom theme: verbose text labels. Demonstrates returning
/// owned strings (not just `&'static str`) through `Cow`.
#[derive(Debug)]
struct LabelTheme;

impl IconTheme for LabelTheme {
    fn glyph(&self, role: IconRole) -> IconSpec {
        // External theme → include a `_ =>` fallback for future
        // variants added in a minor release.
        let s: &'static str = match role {
            IconRole::FolderClosed => "[D]",
            IconRole::FolderOpen => "[O]",
            IconRole::File => "[F]",
            IconRole::Error => "[!]",
            IconRole::CaretRight => ">",
            IconRole::CaretDown => "v",
            _ => "?",
        };
        IconSpec::new(s)
    }
}

/// Custom theme: ultra-compact ASCII. Shows that a theme doesn't
/// need to pull in an icon font — plain text works fine if the
/// glyphs exist in the default system font.
#[derive(Debug)]
struct AsciiTheme;

impl IconTheme for AsciiTheme {
    fn glyph(&self, role: IconRole) -> IconSpec {
        let s: &'static str = match role {
            IconRole::FolderClosed => "+",
            IconRole::FolderOpen => "-",
            IconRole::File => ".",
            IconRole::Error => "!",
            IconRole::CaretRight => ">",
            IconRole::CaretDown => "v",
            _ => "?",
        };
        IconSpec::new(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeChoice {
    Unicode,
    Label,
    Ascii,
}

impl ThemeChoice {
    const ALL: [ThemeChoice; 3] = [ThemeChoice::Unicode, ThemeChoice::Label, ThemeChoice::Ascii];

    fn to_theme(self) -> Arc<dyn IconTheme> {
        match self {
            ThemeChoice::Unicode => Arc::new(UnicodeTheme),
            ThemeChoice::Label => Arc::new(LabelTheme),
            ThemeChoice::Ascii => Arc::new(AsciiTheme),
        }
    }
}

impl std::fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ThemeChoice::Unicode => "Unicode",
            ThemeChoice::Label => "Label",
            ThemeChoice::Ascii => "Ascii",
        })
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tree(DirectoryTreeEvent),
    ThemePicked(ThemeChoice),
}

struct App {
    tree: DirectoryTree,
    choice: ThemeChoice,
    root: PathBuf,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let root = resolve_root();
        let tree = DirectoryTree::new(root.clone())
            .with_filter(DirectoryFilter::FilesAndFolders)
            .with_icon_theme(ThemeChoice::Unicode.to_theme());
        let mut app = App {
            tree,
            choice: ThemeChoice::Unicode,
            root: root.clone(),
        };
        let task = app
            .tree
            .update(DirectoryTreeEvent::Toggled(root))
            .map(Message::Tree);
        (app, task)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tree(ev) => self.tree.update(ev).map(Message::Tree),
            Message::ThemePicked(choice) => {
                // Swapping a theme today requires rebuilding the
                // tree (it's set at construction via the builder).
                // We preserve nothing across the swap — a real app
                // would carry selection/expansion forward, but
                // this is a demo so a clean rebuild keeps it
                // short.
                self.choice = choice;
                self.tree = DirectoryTree::new(self.root.clone())
                    .with_filter(DirectoryFilter::FilesAndFolders)
                    .with_icon_theme(choice.to_theme());
                self.tree
                    .update(DirectoryTreeEvent::Toggled(self.root.clone()))
                    .map(Message::Tree)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let picker = pick_list(
            &ThemeChoice::ALL[..],
            Some(self.choice),
            Message::ThemePicked,
        );
        let header = row![text("Theme:").size(13), picker].spacing(8);

        column![
            header,
            container(self.tree.view(Message::Tree))
                .width(Length::Fill)
                .height(Length::Fill),
            text(
                "Switch themes to see the icon trait in action. \
                 The 'Label' and 'Ascii' themes are defined in \
                 this example file; 'Unicode' is shipped with \
                 the crate."
            )
            .size(12),
        ]
        .spacing(8)
        .padding(10)
        .into()
    }
}

fn resolve_root() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    let scratch = std::env::temp_dir().join("iced-swdir-tree-icon-theme-demo");
    let _ = std::fs::create_dir_all(&scratch);
    for dir in &["src", "src/widgets", "tests", "docs"] {
        let _ = std::fs::create_dir_all(scratch.join(dir));
    }
    for (path, body) in &[
        ("Cargo.toml", "[package]\n"),
        ("README.md", "demo\n"),
        ("src/main.rs", "fn main() {}\n"),
        ("src/widgets/button.rs", ""),
        ("tests/basic.rs", ""),
        ("docs/ROADMAP.md", ""),
    ] {
        let p = scratch.join(path);
        if !p.exists() {
            let _ = std::fs::write(p, body);
        }
    }
    scratch
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("iced-swdir-tree · icon-theme example")
        .run()
}
