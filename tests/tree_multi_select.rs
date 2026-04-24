//! Tree-layer integration: v0.3 multi-select against real
//! filesystem trees. Exercises the full `SelectionMode` matrix
//! (Replace / Toggle / ExtendRange) plus the v0.3 invariant that
//! every selected path survives a filter flip.

mod common;
use common::{TmpDir, find_in_tree};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, SelectionMode};

/// Populate a directory with `a`, `b`, `c` siblings (all files) and
/// return its path + the tree with the root expanded.
fn tree_with_abc(tag: &str) -> (TmpDir, DirectoryTree) {
    let td = TmpDir::new(tag);
    fs::write(td.path().join("a"), b"").unwrap();
    fs::write(td.path().join("b"), b"").unwrap();
    fs::write(td.path().join("c"), b"").unwrap();
    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    (td, tree)
}

#[test]
fn multi_select_toggle_builds_up_a_set() {
    let (td, mut tree) = tree_with_abc("mselect-toggle");
    let a = td.path().join("a");
    let b = td.path().join("b");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 2);
    assert!(tree.is_selected(&a));
    assert!(tree.is_selected(&b));

    // Untoggling /a shrinks the set but keeps /b.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 1);
    assert!(!tree.is_selected(&a));
    assert!(tree.is_selected(&b));
}

#[test]
fn multi_select_range_covers_siblings() {
    let (td, mut tree) = tree_with_abc("mselect-range");
    let a = td.path().join("a");
    let c = td.path().join("c");

    // Anchor at /a, then Shift-range to /c. Expect {/a, /b, /c}.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        c.clone(),
        false,
        SelectionMode::ExtendRange,
    ));
    assert_eq!(tree.selected_paths().len(), 3);
    let names: Vec<_> = tree
        .selected_paths()
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, vec!["a", "b", "c"]);
    // Anchor stayed on /a so a subsequent Shift+click would still
    // range from /a.
    assert_eq!(tree.anchor_path(), Some(a.as_path()));
    // And the per-node flags are all marked true.
    for name in &["a", "b", "c"] {
        let node = find_in_tree(&tree, &td.path().join(name)).unwrap();
        assert!(node.is_selected, "{name} must be visually selected");
    }
}

#[test]
fn filter_change_preserves_every_selected_path() {
    // Multi-select survival: select several paths, flip the filter,
    // confirm every selected path is still in `selected_paths`.
    let td = TmpDir::new("mselect-filter");
    fs::write(td.path().join("a"), b"").unwrap();
    fs::write(td.path().join("b"), b"").unwrap();
    fs::write(td.path().join(".hidden"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let a = td.path().join("a");
    let b = td.path().join("b");
    let hidden = td.path().join(".hidden");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        hidden.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 3);

    // Flip filter — .hidden gets hidden, but stays in the set.
    tree.set_filter(DirectoryFilter::FilesAndFolders);
    assert_eq!(
        tree.selected_paths().len(),
        3,
        "v0.3: every selected path survives filter change"
    );
    assert!(tree.is_selected(&hidden));

    // The per-node view flags are correct on surviving nodes.
    assert!(find_in_tree(&tree, &a).unwrap().is_selected);
    assert!(find_in_tree(&tree, &b).unwrap().is_selected);
    // And .hidden is not currently in the tree (filtered out),
    // but re-appears with its flag restored on the next flip back.
    assert!(find_in_tree(&tree, &hidden).is_none());
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    assert!(find_in_tree(&tree, &hidden).unwrap().is_selected);
}

#[test]
fn selected_path_returns_most_recent_action_target() {
    // The v0.2 single-select accessor still works in v0.3 and returns
    // the "active" path = target of the most recent Replace/Toggle/Range.
    let (td, mut tree) = tree_with_abc("mselect-active");
    let a = td.path().join("a");
    let b = td.path().join("b");
    let c = td.path().join("c");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(a.as_path()));

    let _ = tree.update(DirectoryTreeEvent::Selected(
        b.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_path(), Some(b.as_path()));

    // ExtendRange also updates active, even though anchor stays put.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        c.clone(),
        false,
        SelectionMode::ExtendRange,
    ));
    assert_eq!(tree.selected_path(), Some(c.as_path()));
    assert_eq!(
        tree.anchor_path(),
        Some(b.as_path()),
        "anchor is last Replace/Toggle, not ExtendRange"
    );
}
