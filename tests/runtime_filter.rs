//! Integration: runtime `set_filter` changes visibility without a
//! rescan, and the selection cursor survives the flip (v0.2+).

mod common;
use common::TmpDir;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn set_filter_at_runtime_changes_visibility_without_rescan() {
    let td = TmpDir::new("setfilter");
    td.mkdir("dir");
    td.touch("file.txt");
    td.touch(".hidden");

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    tree.__test_expand_blocking(td.path().to_path_buf());

    let hidden = td.path().join(".hidden");
    let visible = td.path().join("file.txt");

    // Select a visible-under-All entry and confirm selection.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(hidden.as_path()));

    // Flip to a filter that hides dotfiles — the tree re-derives
    // children from its cache, no filesystem I/O required.
    tree.set_filter(DirectoryFilter::FilesAndFolders);

    // **v0.2 change**: selection is per-path and survives filter
    // changes, even when the selected node is filtered out of the
    // visible tree. A stray selection click on an invisible path
    // is still a no-op (it leaves the cursor unchanged), but the
    // cursor value itself persists.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(
        tree.selected_path(),
        Some(hidden.as_path()),
        "v0.2 keeps the selection cursor around through filter changes"
    );

    // Re-selecting a visible sibling replaces it — normal behaviour.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        visible.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(visible.as_path()));

    // Finally, flipping the filter back to AllIncludingHidden means
    // the previously-hidden selection cursor becomes reachable again.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    // The selection cursor is currently on `visible.txt`, not
    // `.hidden`, because we moved it above. Confirm:
    assert_eq!(tree.selected_path(), Some(visible.as_path()));
    // And the per-node flag on visible should be set, confirming
    // `sync_selection_flag` ran.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(
        tree.selected_path(),
        Some(hidden.as_path()),
        "hidden re-selectable once AllIncludingHidden is active again"
    );
}
