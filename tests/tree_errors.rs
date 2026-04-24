//! Tree-layer integration: nonexistent paths and permission denials
//! surface as `node.error` — the widget never panics on I/O failures.

mod common;
use common::{TmpDir, find_in_tree, is_root};

use std::fs;
use std::path::Path;

use iced_swdir_tree::{DirectoryTree, Error};

#[test]
fn nonexistent_path_surfaces_as_error_not_panic() {
    let missing = std::env::temp_dir().join(format!(
        "iced-swdir-tree-nothing-here-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&missing);
    let mut tree = DirectoryTree::new(missing.clone());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, missing.clone());

    let root = find_in_tree(&tree, &missing).expect("root always present");
    assert!(
        root.error.is_some(),
        "missing path must surface as node.error, not a panic or empty list"
    );
    // And a specific ErrorKind so downstream apps can branch on it.
    if let Some(Error::Io { kind, .. }) = root.error.clone() {
        assert_eq!(kind, std::io::ErrorKind::NotFound);
    } else {
        panic!("expected Error::Io");
    }
}

#[cfg(unix)]
#[test]
fn permission_denied_is_greyed_out_not_fatal() {
    use std::os::unix::fs::PermissionsExt;

    // Skip the test if running as root — chmod 0o000 is bypassed by
    // CAP_DAC_OVERRIDE, which most CI runners have. Rootless runs
    // still exercise the code path.
    if is_root() {
        eprintln!("skipping permission test under root (CAP_DAC_OVERRIDE bypasses chmod)");
        return;
    }

    let td = TmpDir::new("perm");
    let locked = td.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Restore perms before the temp dir is dropped, so cleanup works.
    struct Restorer<'a>(&'a Path);
    impl Drop for Restorer<'_> {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
        }
    }
    let _restorer = Restorer(&locked);

    let mut tree = DirectoryTree::new(locked.clone());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, locked.clone());

    let node = find_in_tree(&tree, &locked).expect("locked dir tracked");
    assert!(
        node.error.is_some(),
        "permission denied must surface as error"
    );
    if let Some(Error::Io { kind, .. }) = node.error.clone() {
        assert_eq!(kind, std::io::ErrorKind::PermissionDenied);
    } else {
        panic!("expected Error::Io, got {:?}", node.error);
    }
}
