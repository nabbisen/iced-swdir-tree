//! Integration: the widget survives I/O errors and stale messages
//! without panicking — nonexistent root, unknown paths, permission
//! denied.

mod common;
use common::{TmpDir, is_root};

use std::fs;
use std::path::PathBuf;

use iced_swdir_tree::{DirectoryTree, DirectoryTreeEvent, SelectionMode};

#[test]
fn nonexistent_root_does_not_crash() {
    let bogus = PathBuf::from("/this/path/should/not/exist/for/iced-swdir-tree-tests");
    let mut tree = DirectoryTree::new(bogus.clone());
    // Scanning a nonexistent path yields Err(..) internally; the
    // test helper routes that through the same code path the async
    // scan would. The tree must survive and expose no children.
    tree.__test_expand_blocking(bogus.clone());

    // No selection should be reachable — the root has no children
    // and no file to select other than itself.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        bogus.join("anything"),
        false,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), None);
}

#[test]
fn toggling_an_unknown_path_is_a_noop() {
    let td = TmpDir::new("noop");
    let mut tree = DirectoryTree::new(td.path().to_path_buf());

    // Toggled for a path we've never heard of — widget must not
    // panic and must not spawn a scan.
    let t = tree.update(DirectoryTreeEvent::Toggled(PathBuf::from(
        "/some/other/place",
    )));
    assert_eq!(t.units(), 0);
}

#[cfg(unix)]
#[test]
fn permission_denied_surfaces_as_error_node() {
    use std::os::unix::fs::PermissionsExt;

    // Root can read anything regardless of permissions, so this
    // test becomes a no-op under root. Skip gracefully.
    if is_root() {
        eprintln!("skipping permission_denied_surfaces_as_error_node: running as root");
        return;
    }

    let td = TmpDir::new("noperm");
    let sub = td.mkdir("forbidden");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    tree.__test_expand_blocking(td.path().to_path_buf());

    // Try to expand the forbidden folder. It must not crash.
    tree.__test_expand_blocking(sub.clone());

    // Selecting `sub` still works — the node exists, it's just
    // marked with an error. This is the "gray out the directory"
    // UI behaviour from the spec.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        sub.clone(),
        true,
        SelectionMode::Replace,
    ));
    assert_eq!(tree.selected_path(), Some(sub.as_path()));

    // Restore permissions before the Drop so cleanup works.
    let _ = fs::set_permissions(&sub, fs::Permissions::from_mode(0o700));
}
