//! Tree-layer integration: v0.4 drag-and-drop state-machine
//! behaviour, driven through the public event API against real
//! filesystem fixtures. Covers idle state, press-starts-drag,
//! hover/exit target tracking, Escape cancellation, release
//! state-clearing, multi-source drag, and the descendant-rejection
//! rule.

mod common;
use common::TmpDir;

use std::fs;

use iced_swdir_tree::{DirectoryFilter, DirectoryTree, DirectoryTreeEvent, DragMsg, SelectionMode};

/// Build a tree rooted at a temp dir with subfolders `a/`, `b/` and
/// a file `f.txt`, all fully loaded and the root expanded. Used by
/// drag-and-drop integration tests so we're exercising real
/// filesystem scanning rather than hand-crafted nodes.
fn tree_with_fs_abc(tag: &str) -> (TmpDir, DirectoryTree) {
    let tmp = TmpDir::new(tag);
    fs::create_dir(tmp.path().join("a")).unwrap();
    fs::create_dir(tmp.path().join("b")).unwrap();
    fs::write(tmp.path().join("f.txt"), b"contents").unwrap();
    let mut tree =
        DirectoryTree::new(tmp.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, tmp.path().to_path_buf());
    (tmp, tree)
}

#[test]
fn drag_is_initially_inactive() {
    let (_tmp, tree) = tree_with_fs_abc("drag-idle");
    assert!(!tree.is_dragging());
    assert_eq!(tree.drop_target(), None);
    assert!(tree.drag_sources().is_empty());
}

#[test]
fn press_on_file_row_starts_drag_with_single_source() {
    let (tmp, mut tree) = tree_with_fs_abc("drag-press-file");
    let f = tmp.path().join("f.txt");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f.clone(), false)));
    assert!(tree.is_dragging());
    assert_eq!(tree.drag_sources(), &[f]);
    assert_eq!(tree.drop_target(), None);
}

#[test]
fn hover_over_sibling_folder_sets_drop_target() {
    let (tmp, mut tree) = tree_with_fs_abc("drag-hover-sibling");
    let f = tmp.path().join("f.txt");
    let a = tmp.path().join("a");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f, false)));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(a.clone())));
    assert_eq!(tree.drop_target(), Some(a.as_path()));
}

#[test]
fn hover_over_file_leaves_drop_target_unset() {
    let (tmp, mut tree) = tree_with_fs_abc("drag-hover-file");
    let a = tmp.path().join("a");
    let f = tmp.path().join("f.txt");
    // Drag the folder a, hover over the file f → invalid drop target.
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(a, true)));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(f)));
    assert_eq!(tree.drop_target(), None);
}

#[test]
fn exiting_the_hovered_row_clears_the_drop_target() {
    let (tmp, mut tree) = tree_with_fs_abc("drag-exit");
    let f = tmp.path().join("f.txt");
    let a = tmp.path().join("a");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f, false)));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(a.clone())));
    assert_eq!(tree.drop_target(), Some(a.as_path()));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Exited(a)));
    assert_eq!(tree.drop_target(), None);
}

#[test]
fn escape_cancels_drag() {
    use iced::keyboard::{Key, Modifiers, key::Named};
    let (tmp, mut tree) = tree_with_fs_abc("drag-escape");
    let f = tmp.path().join("f.txt");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f, false)));
    assert!(tree.is_dragging());
    // Simulate Escape through the public key handler.
    let ev = tree
        .handle_key(&Key::Named(Named::Escape), Modifiers::default())
        .expect("Escape during drag must produce a Cancelled event");
    // Route it back through update, as the app would.
    let _ = tree.update(ev);
    assert!(!tree.is_dragging());
    assert_eq!(tree.drop_target(), None);
}

#[test]
fn escape_without_drag_is_unbound() {
    use iced::keyboard::{Key, Modifiers, key::Named};
    let (_tmp, tree) = tree_with_fs_abc("drag-escape-idle");
    assert!(
        tree.handle_key(&Key::Named(Named::Escape), Modifiers::default())
            .is_none(),
        "Escape must be unbound when there's no drag, so apps can \
         still use Escape for their own UI"
    );
}

#[test]
fn release_over_valid_target_clears_drag_state() {
    // We can't inspect the `Task<DragCompleted>` the widget returns
    // without an iced runtime, but we *can* verify the state
    // machine transitions correctly: after Release, no drag is
    // active and no drop target is hovered. The delayed
    // DragCompleted event is exercised end-to-end by
    // `examples/drag_drop.rs`.
    let (tmp, mut tree) = tree_with_fs_abc("drag-release-ok");
    let f = tmp.path().join("f.txt");
    let a = tmp.path().join("a");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f, false)));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(a.clone())));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(a)));
    assert!(!tree.is_dragging());
    assert_eq!(tree.drop_target(), None);
}

#[test]
fn release_on_same_row_leaves_state_clean() {
    // Press and release on the same row is a click, not a drag.
    // Drag state clears; a Selected event is dispatched via
    // Task::done inside the returned task (also not directly
    // inspectable here).
    let (tmp, mut tree) = tree_with_fs_abc("drag-release-click");
    let f = tmp.path().join("f.txt");
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f.clone(), false)));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Released(f)));
    assert!(!tree.is_dragging());
    // Critical: selection was NOT mutated directly. It would only
    // be set once the app processes the delayed `Selected` event
    // carried in the returned Task — which we can't observe here.
    assert!(tree.selected_paths().is_empty());
}

#[test]
fn pressing_on_selected_row_drags_whole_selection() {
    let (tmp, mut tree) = tree_with_fs_abc("drag-multi");
    let a = tmp.path().join("a");
    let f = tmp.path().join("f.txt");
    // Build a 2-item multi-selection: {a, f}.
    let _ = tree.update(DirectoryTreeEvent::Selected(
        a.clone(),
        true,
        SelectionMode::Replace,
    ));
    let _ = tree.update(DirectoryTreeEvent::Selected(
        f.clone(),
        false,
        SelectionMode::Toggle,
    ));
    assert_eq!(tree.selected_paths().len(), 2);
    // Press on `f` (which is in the selection): drag should carry
    // both.
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(f, false)));
    let sources = tree.drag_sources();
    assert_eq!(sources.len(), 2, "expected both selected paths");
    assert!(sources.contains(&a));
}

#[test]
fn descendant_of_source_is_not_a_valid_drop_target() {
    // Create /r/parent/child with parent already expanded+loaded.
    let tmp = TmpDir::new("drag-descendant");
    fs::create_dir_all(tmp.path().join("parent/child")).unwrap();
    let mut tree =
        DirectoryTree::new(tmp.path().to_path_buf()).with_filter(DirectoryFilter::FilesAndFolders);
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, tmp.path().to_path_buf());
    let parent = tmp.path().join("parent");
    iced_swdir_tree::__testing::scan_and_feed(&mut tree, parent.clone());
    // Expand parent so its child is visible.
    let _ = tree.update(DirectoryTreeEvent::Toggled(parent.clone()));
    let child = parent.join("child");

    // Drag `parent` onto its own `child` — must NOT become a
    // valid drop target.
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Pressed(
        parent.clone(),
        true,
    )));
    let _ = tree.update(DirectoryTreeEvent::Drag(DragMsg::Entered(child)));
    assert_eq!(
        tree.drop_target(),
        None,
        "dropping a folder into its own descendant must be rejected"
    );
}
