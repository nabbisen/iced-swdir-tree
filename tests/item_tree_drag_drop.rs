//! Integration: `ItemTree<T>` drag-and-drop state machine (RFC 002),
//! driven through the public event API against hand-built in-memory
//! trees. `ItemTree` is synchronous, so — unlike `DirectoryTree` —
//! these need no executor shim and no filesystem fixtures.
//!
//! As with the directory widget, the `Task` the widget returns on a
//! click or drop is opaque without an iced runtime, so these verify
//! the *state-machine transitions* (and that selection is never
//! mutated directly on press). The end-to-end `DragCompleted` /
//! deferred-`Selected` effects are exercised by
//! `examples/item_tree.rs`.

use iced::keyboard::{Key, Modifiers, key::Named};
use iced_swdir_tree::{
    DropPosition, ItemDragMsg, ItemNode, ItemTree, ItemTreeEvent, NodeId, SelectionMode,
};

/// A fixed test tree:
///
/// ```text
/// 0 (root)
/// ├── 1
/// │   ├── 11
/// │   └── 12
/// └── 2
///     └── 21
/// ```
fn tree() -> ItemTree<String> {
    fn n(id: u64, children: Vec<ItemNode<String>>) -> ItemNode<String> {
        ItemNode {
            id: NodeId(id),
            data: format!("node {id}"),
            children,
        }
    }
    let mut t = ItemTree::new().with_drag_and_drop(true);
    t.set_tree(n(
        0,
        vec![
            n(1, vec![n(11, vec![]), n(12, vec![])]),
            n(2, vec![n(21, vec![])]),
        ],
    ));
    t
}

fn drag(t: &mut ItemTree<String>, msg: ItemDragMsg) {
    let _ = t.update(ItemTreeEvent::Drag(msg));
}

#[test]
fn drag_is_initially_inactive() {
    let t = tree();
    assert!(t.is_drag_and_drop_enabled());
    assert!(!t.is_dragging());
    assert_eq!(t.drop_target(), None);
    assert!(t.drag_sources().is_empty());
}

#[test]
fn drag_disabled_ignores_press() {
    let mut t = tree().with_drag_and_drop(false);
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert!(!t.is_dragging(), "press must be a no-op when DnD is off");
}

#[test]
fn press_starts_drag_with_single_source() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert!(t.is_dragging());
    assert_eq!(t.drag_sources(), &[NodeId(11)]);
    assert_eq!(t.drop_target(), None);
}

#[test]
fn hover_over_valid_sibling_sets_drop_target() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::Before),
    );
    assert_eq!(t.drop_target(), Some((NodeId(12), DropPosition::Before)));
}

#[test]
fn hover_creating_cycle_leaves_target_unset() {
    let mut t = tree();
    // Drag `1`; hovering "into" its own child `11` is a cycle.
    drag(&mut t, ItemDragMsg::Pressed(NodeId(1)));
    drag(&mut t, ItemDragMsg::Entered(NodeId(11), DropPosition::Into));
    assert_eq!(t.drop_target(), None);
}

#[test]
fn exiting_hovered_zone_clears_target() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::After),
    );
    assert_eq!(t.drop_target(), Some((NodeId(12), DropPosition::After)));
    drag(&mut t, ItemDragMsg::Exited(NodeId(12), DropPosition::After));
    assert_eq!(t.drop_target(), None);
}

#[test]
fn exit_of_a_different_zone_does_not_clear_target() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::Before),
    );
    // A stray exit for some other zone must not clear the live hover.
    drag(&mut t, ItemDragMsg::Exited(NodeId(2), DropPosition::Into));
    assert_eq!(t.drop_target(), Some((NodeId(12), DropPosition::Before)));
}

#[test]
fn escape_cancels_drag() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert!(t.is_dragging());
    let ev = t
        .handle_key(&Key::Named(Named::Escape), Modifiers::default())
        .expect("Escape during a drag must produce a Cancelled event");
    let _ = t.update(ev);
    assert!(!t.is_dragging());
    assert_eq!(t.drop_target(), None);
}

#[test]
fn escape_without_drag_is_unbound() {
    let t = tree();
    assert!(
        t.handle_key(&Key::Named(Named::Escape), Modifiers::default())
            .is_none(),
        "Escape must be unbound with no drag so apps keep it for their own UI"
    );
}

#[test]
fn release_over_valid_target_clears_state() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::Before),
    );
    drag(
        &mut t,
        ItemDragMsg::Released(NodeId(12), DropPosition::Before),
    );
    assert!(!t.is_dragging());
    assert_eq!(t.drop_target(), None);
}

#[test]
fn release_on_same_node_is_a_click_and_does_not_mutate_selection() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Released(NodeId(11), DropPosition::Into),
    );
    assert!(!t.is_dragging());
    // The deferred Selected rides in the returned Task; the widget
    // must NOT have mutated the selection set directly.
    assert!(t.selected_ids().is_empty());
}

#[test]
fn pressing_a_selected_node_drags_the_whole_selection_in_tree_order() {
    let mut t = tree();
    // Build selection {12, 11} in that click order...
    let _ = t.update(ItemTreeEvent::Selected(NodeId(12), SelectionMode::Replace));
    let _ = t.update(ItemTreeEvent::Selected(NodeId(11), SelectionMode::Toggle));
    assert_eq!(t.selected_ids().len(), 2);
    // ...then press one of them: sources carry both, in TREE order.
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert_eq!(t.drag_sources(), &[NodeId(11), NodeId(12)]);
}

#[test]
fn pressing_an_unselected_node_drags_only_that_node() {
    let mut t = tree();
    let _ = t.update(ItemTreeEvent::Selected(NodeId(1), SelectionMode::Replace));
    // Press a node that is NOT in the selection.
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert_eq!(t.drag_sources(), &[NodeId(11)]);
}

#[test]
fn cancelled_message_clears_state() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(&mut t, ItemDragMsg::Cancelled);
    assert!(!t.is_dragging());
}

#[test]
fn stray_entered_without_press_is_a_noop() {
    let mut t = tree();
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::Before),
    );
    assert!(!t.is_dragging());
    assert_eq!(t.drop_target(), None);
}

// -- composability: set_tree while drag active --

#[test]
fn set_tree_while_drag_active_clears_drag() {
    // The parent-map snapshot taken at Pressed is stale the moment
    // set_tree rebuilds the tree; continuing the drag would produce
    // incorrect validity results. The widget must clear drag state
    // (state-machine.md composability rule).
    fn n(id: u64) -> ItemNode<String> {
        ItemNode {
            id: NodeId(id),
            data: format!("node {id}"),
            children: vec![],
        }
    }
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert!(t.is_dragging());

    t.set_tree(ItemNode {
        id: NodeId(0),
        data: "root".into(),
        children: vec![n(1), n(2)],
    });
    assert!(!t.is_dragging(), "set_tree must clear stale drag state");
    assert_eq!(t.drop_target(), None);
    assert!(t.drag_sources().is_empty());
}

// -- composability: disabling DnD while drag active --

#[test]
fn disabling_dnd_while_drag_active_clears_drag() {
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    assert!(t.is_dragging());
    // with_drag_and_drop is a consuming builder; reassign to disable.
    let t = t.with_drag_and_drop(false);
    assert!(!t.is_dragging(), "disabling DnD must clear in-flight drag");
    assert!(!t.is_drag_and_drop_enabled());
}

// -- composability: set_search_query preserves drag (S11.16) --

#[test]
fn set_search_query_preserves_drag_state() {
    // S11.16: an active drag must survive a search query change.
    // Search re-filters visible rows but must not touch drag state.
    let mut t = tree();
    drag(&mut t, ItemDragMsg::Pressed(NodeId(11)));
    drag(
        &mut t,
        ItemDragMsg::Entered(NodeId(12), DropPosition::Before),
    );
    assert!(t.is_dragging());
    assert_eq!(t.drop_target(), Some((NodeId(12), DropPosition::Before)));

    t.set_search_query("node");

    assert!(t.is_dragging(), "search must not clear drag state");
    assert_eq!(
        t.drop_target(),
        Some((NodeId(12), DropPosition::Before)),
        "hover must survive set_search_query"
    );
}
