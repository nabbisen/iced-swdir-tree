//! Integration: expand, collapse, and re-expand behaviour —
//! including the "re-expand is cached and synchronous" invariant
//! (no second scan is dispatched).

mod common;
use common::TmpDir;

use iced_swdir_tree::{DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn expand_populates_children() {
    let td = TmpDir::new("expand");
    td.mkdir("a");
    td.mkdir("b");
    td.touch("c.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let a = td.path().join("a");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(a.as_path()));

    let c = td.path().join("c.txt");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        c.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(c.as_path()));
}

#[test]
fn collapse_keeps_children_and_reexpand_is_instant() {
    let td = TmpDir::new("recollapse");
    td.mkdir("x");
    td.touch("y.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let x = td.path().join("x");
    let _ = tree.update(DirectoryTreeEvent::Selected(
        x.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(x.as_path()));

    // Collapse the root via Toggled — should be instant, no Task
    // with a Loaded follow-up.
    let t = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(t.units(), 0, "collapse must not spawn an async scan");

    // Re-expand — this time the children are cached, so the
    // expansion is synchronous and emits no Task either.
    let t2 = tree.update(DirectoryTreeEvent::Toggled(td.path().to_path_buf()));
    assert_eq!(
        t2.units(),
        0,
        "re-expanding a cached directory must not spawn another scan"
    );

    // And `x` is still selectable.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        x.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(x.as_path()));
}
