//! Tree-layer integration: the four `DirectoryFilter` variants
//! against a real filesystem.

mod common;
use common::{TmpDir, child_names};

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree};

#[test]
fn files_and_folders_filter_shows_both() {
    let td = TmpDir::new("fandf");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert!(names.contains(&"sub".into()), "folder must be listed");
    assert!(names.contains(&"visible.txt".into()), "file must be listed");
    assert_eq!(names.len(), 2);
}

#[test]
fn folders_only_filter_drops_files() {
    let td = TmpDir::new("fonly");
    fs::create_dir(td.path().join("sub")).unwrap();
    fs::write(td.path().join("file.txt"), b"").unwrap();

    let mut tree =
        DirectoryTree::new(td.path().to_path_buf()).with_filter(DirectoryFilter::FoldersOnly);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "sub");
}

#[test]
fn all_including_hidden_shows_dotfiles() {
    let td = TmpDir::new("hidden");
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf())
        .with_filter(DirectoryFilter::AllIncludingHidden);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&".secret".into()));
}

#[test]
fn default_filter_hides_dotfiles() {
    let td = TmpDir::new("no-dot");
    fs::write(td.path().join(".secret"), b"").unwrap();
    fs::write(td.path().join("visible.txt"), b"").unwrap();

    let mut tree = DirectoryTree::new(td.path().to_path_buf());
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, td.path().to_path_buf());

    let names = child_names(&tree);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "visible.txt");
}
