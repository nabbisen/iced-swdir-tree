//! Tree-layer integration: state-preservation invariants — the
//! widget's cache layer is supposed to keep expansion state,
//! loaded-flag state, and deeper subtree content across filter
//! flips and collapse/re-expand cycles. These tests pin that
//! contract.

mod common;
use common::{TmpDir, child_names, find_in_tree};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent};

#[test]
fn filter_change_rebuilds_from_cache() {
    let td = TmpDir::new("refilter");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    assert_eq!(child_names(&tree).len(), 2);

    // Flip to AllIncludingHidden — must add `.secret` in place
    // without issuing another scan.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);
    assert_eq!(child_names(&tree).len(), 3);
    assert!(child_names(&tree).contains(&".secret".into()));

    // Flip to FoldersOnly — every file disappears.
    tree.set_filter(DirectoryFilter::FoldersOnly);
    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "sub");
}

#[test]
fn collapse_then_reexpand_keeps_children() {
    let td = TmpDir::new("collapse");
    fs::create_dir(td.path().join("a")).unwrap();
    fs::write(td.path().join("b.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    assert_eq!(child_names(&tree).len(), 2);

    // Collapse the root. Children must remain (collapse is instant,
    // no I/O).
    let _ = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    // Re-expand. Must not re-issue a scan (is_loaded stays true);
    // children should still be the same list.
    let _ = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(child_names(&tree).len(), 2);
}

#[test]
fn filter_change_preserves_expanded_subtree() {
    // User has expanded /root/sub and loaded its children; flipping
    // the filter must not collapse /root/sub or re-trigger its scan.
    let td = TmpDir::new("expanded-preserve");
    fs::create_dir_all(td.path().join("sub/inner")).unwrap();
    fs::write(td.path().join("sub/inner/leaf.txt"), b"").unwrap();
    fs::write(td.path().join(".hidden"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());
    // Expand the subdirectory too so there's deeper state to preserve.
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().join("sub"));

    // Pre-condition: sub is expanded and loaded.
    let sub_before = find_in_tree(&tree, &td.path().join("sub")).expect("sub exists");
    assert!(sub_before.is_expanded);
    assert!(sub_before.is_loaded);
    assert!(!sub_before.children.is_empty());

    // Flip the filter. No I/O; tree re-derives from cache.
    tree.set_filter(DirectoryFilter::AllIncludingHidden);

    // Post-condition: sub is still expanded, still loaded, still
    // has its inner child. This is the v0.2 "rebuild_from_cache
    // preserves subtree" contract.
    let sub_after =
        find_in_tree(&tree, &td.path().join("sub")).expect("sub must still be in the tree");
    assert!(
        sub_after.is_expanded,
        "expansion must survive filter change"
    );
    assert!(
        sub_after.is_loaded,
        "loaded flag must survive filter change"
    );
    assert!(
        !sub_after.children.is_empty(),
        "deeper subtree must survive filter change"
    );
}
