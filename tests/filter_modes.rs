//! Integration: the three `DirectoryFilter` variants, each driven
//! against a real filesystem fixture.

mod common;
use common::TmpDir;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn filter_folders_only_hides_files_after_expand() {
    let td = TmpDir::new("folders-only");
    td.mkdir("sub");
    td.touch("note.txt");

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FoldersOnly);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let sub = td.path().join("sub");
    let note = td.path().join("note.txt");

    // `sub` is a folder — visible under FoldersOnly.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        sub.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(sub.as_path()));

    // `note.txt` is filtered out — selecting it must be a no-op:
    // the filtered node is not reachable, and the previous
    // selection stays put.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        note.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(
        tree.selected_path(),
        Some(sub.as_path()),
        "stray selection on a filtered-out path must be ignored"
    );
}

#[test]
fn filter_files_and_folders_hides_hidden_entries() {
    let td = TmpDir::new("no-hidden");
    td.touch(".hidden");
    td.touch("visible.txt");

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let visible = td.path().join("visible.txt");
    let hidden = td.path().join(".hidden");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        visible.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(visible.as_path()));

    // Stray selection on a filtered-out path is a no-op — the
    // previous selection stays intact.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(
        tree.selected_path(),
        Some(visible.as_path()),
        "hidden file should not be selectable under FilesAndFolders"
    );
}

#[test]
fn filter_all_includes_hidden_entries() {
    let td = TmpDir::new("with-hidden");
    td.touch(".hidden");
    td.touch("visible.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let hidden = td.path().join(".hidden");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(hidden.as_path()));
}
