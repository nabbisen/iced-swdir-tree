//! Integration: basic single-select behaviour under
//! `SelectionMode::Replace`.

mod common;
use common::TmpDir;

use iced_swdir_tree::{DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn selection_is_single_select() {
    let td = TmpDir::new("select");
    td.touch("one.txt");
    td.touch("two.txt");

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    let one = td.path().join("one.txt");
    let two = td.path().join("two.txt");

    let _ = tree.update(DirectoryTreeEvent::Selected(
        one.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(one.as_path()));

    let _ = tree.update(DirectoryTreeEvent::Selected(
        two.clone(),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(two.as_path()));
}
