//! Tree-layer integration: single-selection behaviour against the
//! real filesystem, including the v0.2 guarantee that the selection
//! cursor survives filter changes that hide (or keep) the target.

mod common;
use common::{TmpDir, find_in_tree};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn selection_lands_on_the_target_path() {
    let td = TmpDir::new("select");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::create_dir(td.path().join("also")).unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let target = td.path().join("keeps");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));
}

#[test]
fn filter_change_preserves_selection_cursor() {
    // **v0.2 behaviour**: selection is per-path and survives filter
    // changes, even when the filter hides the selected node.
    let td = TmpDir::new("select-filter");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    // Select the file, then flip to a filter that hides files.
    let target = td.path().join("file.txt");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));

    tree.set_filter(DirectoryFilter::FoldersOnly);

    // The cursor persists even though the file is no longer visible.
    assert_eq!(
        tree.selected_path(),
        Some(target.as_path()),
        "v0.2: selection cursor survives filter change"
    );

    // Flipping the filter back to FilesAndFolders and re-selecting
    // the path confirms it is reachable again.
    tree.set_filter(DirectoryFilter::FilesAndFolders);
    assert_eq!(tree.selected_path(), Some(target.as_path()));
}

#[test]
fn filter_change_preserves_directory_selection_when_still_visible() {
    // Companion to the previous test: if the selected node is
    // *still* visible under the new filter, the per-node
    // is_selected flag is re-synced to match.
    let td = TmpDir::new("select-folder-filter");
    fs::create_dir(td.path().join("keeps")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let target = td.path().join("keeps");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        target.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(target.as_path()));

    tree.set_filter(DirectoryFilter::FoldersOnly);
    assert_eq!(tree.selected_path(), Some(target.as_path()));
    // And the per-node cache is re-marked on the freshly built node.
    let node = find_in_tree(&tree, &target).expect("folder must still be in the tree");
    assert!(
        node.is_selected,
        "per-node flag must be re-synced after rebuild"
    );
}
