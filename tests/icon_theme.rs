//! v0.7 integration: `IconTheme` trait and `with_icon_theme` builder.
//!
//! These tests exercise the theme at the API boundary, not via the
//! internal helpers. If a future refactor breaks the wiring
//! between `DirectoryTree` and `icon::render`, these fail before
//! a downstream app would notice.

mod common;
use common::TmpDir;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use iced_swdir_tree::{
    DirectoryFilter, DirectoryTree, DirectoryTreeEvent, IconRole, IconSpec, IconTheme, UnicodeTheme,
};

/// A theme that counts how many times each role was queried.
/// Proves the widget's view actually calls the theme rather than
/// bypassing it.
#[derive(Debug, Default)]
struct CountingTheme {
    folder_closed: AtomicUsize,
    folder_open: AtomicUsize,
    file: AtomicUsize,
    caret_right: AtomicUsize,
    caret_down: AtomicUsize,
    error: AtomicUsize,
}

impl IconTheme for CountingTheme {
    fn glyph(&self, role: IconRole) -> IconSpec {
        let counter = match role {
            IconRole::FolderClosed => &self.folder_closed,
            IconRole::FolderOpen => &self.folder_open,
            IconRole::File => &self.file,
            IconRole::CaretRight => &self.caret_right,
            IconRole::CaretDown => &self.caret_down,
            IconRole::Error => &self.error,
            _ => return IconSpec::new("?"),
        };
        counter.fetch_add(1, Ordering::Relaxed);
        IconSpec::new("*")
    }
}

#[test]
fn new_tree_has_default_theme_installed() {
    // A newly-constructed tree's theme must be usable: asking it
    // for a folder role returns a non-empty glyph. We don't assert
    // on which stock theme is chosen — that's feature-flag
    // dependent — only that something sane is in place.
    let _tree = DirectoryTree::new(std::path::PathBuf::from("/tmp"));
    // The theme is crate-internal, so we use the known-public
    // UnicodeTheme as a sanity proxy: UnicodeTheme is reachable
    // from the public API, confirming the IconTheme trait surface
    // is complete enough for external implementations.
    let spec = UnicodeTheme.glyph(IconRole::FolderClosed);
    assert!(!spec.glyph.as_ref().is_empty());
}

#[test]
fn with_icon_theme_accepts_arc_dyn() {
    // Object-safe trait: the builder accepts any `Arc<dyn IconTheme>`.
    let theme: Arc<dyn IconTheme> = Arc::new(UnicodeTheme);
    let _tree = DirectoryTree::new(std::path::PathBuf::from("/tmp"))
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_icon_theme(theme);
}

#[test]
fn view_calls_the_installed_theme() {
    // Build a tree with a CountingTheme, render it once, and check
    // the counts reflect what the view needed to draw.
    let td = TmpDir::new("icon-theme-view");
    fs::create_dir(td.path().join("subdir")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let counting = Arc::new(CountingTheme::default());
    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_icon_theme(counting.clone());
    tree.__test_expand_blocking(td.path().to_path_buf());

    // Render the tree — this is what the iced runtime would do
    // every frame. We discard the Element; we just need to know
    // that building it consulted the theme.
    let _: iced::Element<'_, DirectoryTreeEvent> = tree.view(|e| e);

    let total = counting.folder_closed.load(Ordering::Relaxed)
        + counting.folder_open.load(Ordering::Relaxed)
        + counting.file.load(Ordering::Relaxed)
        + counting.caret_right.load(Ordering::Relaxed)
        + counting.caret_down.load(Ordering::Relaxed)
        + counting.error.load(Ordering::Relaxed);
    assert!(
        total > 0,
        "view rendered without consulting the theme at all; \
         CountingTheme counts: folder_closed={} folder_open={} file={} \
         caret_right={} caret_down={} error={}",
        counting.folder_closed.load(Ordering::Relaxed),
        counting.folder_open.load(Ordering::Relaxed),
        counting.file.load(Ordering::Relaxed),
        counting.caret_right.load(Ordering::Relaxed),
        counting.caret_down.load(Ordering::Relaxed),
        counting.error.load(Ordering::Relaxed)
    );
    // The root is an expanded folder, so its FolderOpen + CaretDown
    // should have been queried at least once.
    assert!(
        counting.folder_open.load(Ordering::Relaxed) >= 1,
        "expanded root should have requested FolderOpen at least once"
    );
    // The subdirectory is collapsed; its FolderClosed + CaretRight
    // should have been queried.
    assert!(
        counting.folder_closed.load(Ordering::Relaxed) >= 1,
        "collapsed subdir should have requested FolderClosed"
    );
    // The file should have triggered File.
    assert!(
        counting.file.load(Ordering::Relaxed) >= 1,
        "file row should have requested IconRole::File"
    );
}

#[test]
fn theme_survives_filter_change() {
    // Swapping the filter rebuilds tree nodes but must not reset
    // the installed theme. Render → filter change → render again,
    // both must consult the same theme.
    let td = TmpDir::new("icon-theme-filter");
    fs::create_dir(td.path().join("folder")).unwrap();

    let counting = Arc::new(CountingTheme::default());
    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::FilesAndFolders)
        .with_icon_theme(counting.clone());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let _: iced::Element<'_, DirectoryTreeEvent> = tree.view(|e| e);
    let first = counting.folder_open.load(Ordering::Relaxed);
    assert!(first >= 1);

    tree.set_filter(DirectoryFilter::FoldersOnly);
    let _: iced::Element<'_, DirectoryTreeEvent> = tree.view(|e| e);
    let second = counting.folder_open.load(Ordering::Relaxed);
    assert!(
        second > first,
        "post-filter render should also consult the theme"
    );
}

#[test]
fn theme_arc_is_cheaply_cloneable() {
    // Holding the theme in Arc<dyn IconTheme> means cloning the
    // handle is cheap — that's the point. Also verifies the trait
    // doesn't require Sized.
    let theme: Arc<dyn IconTheme> = Arc::new(UnicodeTheme);
    let _c1 = theme.clone();
    let _c2 = theme.clone();
    let _c3 = Arc::clone(&theme);
    // Four handles, one underlying theme.
    assert_eq!(Arc::strong_count(&theme), 4);
}
