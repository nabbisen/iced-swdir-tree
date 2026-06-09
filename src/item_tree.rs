//! `ItemTree<T>` — a keyboard-navigable, multi-selectable,
//! searchable tree widget for **in-memory, caller-supplied** node
//! data.
//!
//! # Quick start
//!
//! ```no_run
//! use iced_swdir_tree::{ItemTree, ItemNode, NodeId};
//!
//! let mut tree: ItemTree<String> = ItemTree::new();
//! tree.set_tree(ItemNode {
//!     id: NodeId(0),
//!     data: "Root".into(),
//!     children: vec![
//!         ItemNode { id: NodeId(1), data: "Chapter 1".into(), children: vec![] },
//!         ItemNode { id: NodeId(2), data: "Chapter 2".into(), children: vec![] },
//!     ],
//! });
//! ```
//!
//! # Differences from `DirectoryTree`
//!
//! | `DirectoryTree` | `ItemTree<T>` |
//! |---|---|
//! | Node identity: `PathBuf` | Node identity: `NodeId(u64)` |
//! | Data loaded asynchronously via `ScanExecutor` | Data always fully present |
//! | `Toggled` → scan → `Loaded` lifecycle | No loading step |
//! | `generation` counter guards stale scan results | No generation counter |
//! | Drag-and-drop | Deferred to v0.8.x |
//!
//! Navigation, multi-select, search, and icon themes are identical.
use std::collections::HashSet;
use std::fmt::Display;
use std::sync::Arc;

use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Task};

pub(crate) mod node;
pub(crate) mod search;

#[cfg(test)]
mod tests;

use node::{
    ItemNode, ItemNodeState, NodeId, VisibleRow, clear_selection, collect_all_ids,
    collect_search_visible, collect_visible, find, find_mut, parent_of, snapshot_state,
};
use search::{ItemSearchState, walk_for_search};

use crate::directory_tree::icon::{IconRole, IconTheme, default_theme, render as icon_render};
use crate::directory_tree::selection::SelectionMode;

// ----------------------------------------------------------------
// Event
// ----------------------------------------------------------------

/// Events emitted by [`ItemTree`].
///
/// Route all of these back through
/// [`ItemTree::update`] in your application's `update` function.
#[derive(Debug, Clone)]
pub enum ItemTreeEvent {
    /// The user clicked the caret on a branch node.
    Toggled(NodeId),
    /// The user selected a row.
    Selected(NodeId, SelectionMode),
}

// ----------------------------------------------------------------
// Widget
// ----------------------------------------------------------------

/// A keyboard-navigable, multi-selectable, searchable tree widget
/// for in-memory, caller-supplied node data.
///
/// See the crate-level module documentation for a quick start and
/// a comparison with [`DirectoryTree`](crate::DirectoryTree).
#[derive(Debug)]
pub struct ItemTree<T> {
    root: Option<ItemNodeState<T>>,
    selected_ids: Vec<NodeId>,
    active_id: Option<NodeId>,
    anchor_id: Option<NodeId>,
    search: Option<ItemSearchState>,
    icon_theme: Arc<dyn IconTheme>,
}

impl<T: Clone + std::fmt::Debug + Send + Sync + 'static> ItemTree<T> {
    // ---- construction ----

    /// Create an empty `ItemTree`. Call `set_tree`
    /// to populate it.
    pub fn new() -> Self {
        Self {
            root: None,
            selected_ids: Vec::new(),
            active_id: None,
            anchor_id: None,
            search: None,
            icon_theme: default_theme(),
        }
    }

    /// Replace the icon theme.
    ///
    /// The crate ships [`UnicodeTheme`](crate::UnicodeTheme) (always
    /// available) and [`LucideTheme`](crate::LucideTheme) (behind the
    /// `icons` feature). Pass any `Arc<dyn IconTheme>` here.
    /// Only [`IconRole::CaretRight`] and [`IconRole::CaretDown`] are
    /// used by `ItemTree`.
    pub fn with_icon_theme(mut self, theme: Arc<dyn IconTheme>) -> Self {
        self.icon_theme = theme;
        self
    }

    // ---- data model ----

    /// Replace the tree contents, diffing the new data against the
    /// current tree to preserve expansion and selection state for
    /// surviving [`NodeId`]s.
    ///
    /// Expansion and selection are **preserved** for any node whose
    /// `id` appears anywhere in the new tree (regardless of position
    /// changes). State is **dropped** for any id that disappears —
    /// selected ids that vanish are silently removed from the
    /// selection set.
    pub fn set_tree(&mut self, root: ItemNode<T>) {
        // Snapshot current per-node state keyed by NodeId.
        let mut old_state = std::collections::HashMap::new();
        if let Some(existing) = &self.root {
            snapshot_state(existing, &mut old_state);
        }

        // Build the new internal state tree, transferring old state.
        self.root = Some(ItemNodeState::from_input(root, &old_state));

        // Drop selected/active/anchor ids that no longer exist.
        let mut live_ids = HashSet::new();
        if let Some(r) = &self.root {
            collect_all_ids(r, &mut live_ids);
        }
        self.selected_ids.retain(|id| live_ids.contains(id));
        if self.active_id.is_some_and(|id| !live_ids.contains(&id)) {
            self.active_id = None;
        }
        if self.anchor_id.is_some_and(|id| !live_ids.contains(&id)) {
            self.anchor_id = None;
        }

        // Re-sync view flags.
        self.sync_selection_flags();
        // Note: recompute_search_visibility requires T: Display.
        // Call set_tree_and_recompute_search (below) when T: Display
        // and search may be active.
    }

    fn sync_selection_flags(&mut self) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        clear_selection(root);
        let ids: Vec<NodeId> = self.selected_ids.clone();
        for id in ids {
            if let Some(node) = find_mut(root, id) {
                node.is_selected = true;
            }
        }
    }

    pub(crate) fn visible_rows(&self) -> Vec<VisibleRow<'_, T>> {
        let Some(root) = &self.root else {
            return Vec::new();
        };
        match &self.search {
            None => {
                let mut out = Vec::new();
                collect_visible(root, 0, &mut out);
                out
            }
            Some(state) => {
                let mut out = Vec::new();
                collect_search_visible(root, 0, &state.visible_ids, &mut out);
                out
            }
        }
    }

    /// Apply an [`ItemTreeEvent`], mutate the tree state, and return
    /// a `Task` (always `Task::none()`).
    pub fn update(&mut self, event: ItemTreeEvent) -> Task<ItemTreeEvent> {
        match event {
            ItemTreeEvent::Toggled(id) => self.on_toggled(id),
            ItemTreeEvent::Selected(id, mode) => self.on_selected(id, mode),
        }
        Task::none()
    }

    fn on_toggled(&mut self, id: NodeId) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        let Some(node) = find_mut(root, id) else {
            return;
        };
        if !node.children.is_empty() {
            node.is_expanded = !node.is_expanded;
        }
    }

    fn on_selected(&mut self, id: NodeId, mode: SelectionMode) {
        let Some(root) = self.root.as_ref() else {
            return;
        };
        if find(root, id).is_none() {
            return;
        }
        self.active_id = Some(id);
        match mode {
            SelectionMode::Replace => {
                self.selected_ids = vec![id];
                self.anchor_id = Some(id);
            }
            SelectionMode::Toggle => {
                if let Some(pos) = self.selected_ids.iter().position(|&x| x == id) {
                    self.selected_ids.remove(pos);
                } else {
                    self.selected_ids.push(id);
                }
                self.anchor_id = Some(id);
            }
            SelectionMode::ExtendRange => {
                let rows = self.visible_rows();
                let anchor = self.anchor_id.unwrap_or(id);
                let ai = rows.iter().position(|r| r.node.id == anchor);
                let bi = rows.iter().position(|r| r.node.id == id);
                if let (Some(a), Some(b)) = (ai, bi) {
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    self.selected_ids = rows[lo..=hi].iter().map(|r| r.node.id).collect();
                } else {
                    self.selected_ids = vec![id];
                    self.anchor_id = Some(id);
                }
            }
        }
        self.sync_selection_flags();
    }

    /// Translate a keyboard event into an `ItemTreeEvent`, or `None`.
    pub fn handle_key(&self, key: &Key, modifiers: Modifiers) -> Option<ItemTreeEvent> {
        use iced::keyboard::key::Named;
        let rows = self.visible_rows();
        if rows.is_empty() {
            return None;
        }
        let cur = self
            .active_id
            .and_then(|id| rows.iter().position(|r| r.node.id == id))
            .unwrap_or(0);
        let mode = if modifiers.shift() {
            SelectionMode::ExtendRange
        } else {
            SelectionMode::Replace
        };
        match key {
            Key::Named(Named::ArrowUp) => Some(ItemTreeEvent::Selected(
                rows[cur.saturating_sub(1)].node.id,
                mode,
            )),
            Key::Named(Named::ArrowDown) => Some(ItemTreeEvent::Selected(
                rows[(cur + 1).min(rows.len() - 1)].node.id,
                mode,
            )),
            Key::Named(Named::Home) => Some(ItemTreeEvent::Selected(rows[0].node.id, mode)),
            Key::Named(Named::End) => {
                Some(ItemTreeEvent::Selected(rows[rows.len() - 1].node.id, mode))
            }
            Key::Named(Named::Enter) => {
                let r = &rows[cur];
                (!r.node.children.is_empty()).then_some(ItemTreeEvent::Toggled(r.node.id))
            }
            Key::Named(Named::Space) => Some(ItemTreeEvent::Selected(
                rows[cur].node.id,
                SelectionMode::Toggle,
            )),
            Key::Named(Named::ArrowLeft) => {
                let r = &rows[cur];
                if r.node.is_expanded && !r.node.children.is_empty() {
                    Some(ItemTreeEvent::Toggled(r.node.id))
                } else {
                    self.root
                        .as_ref()
                        .and_then(|root| parent_of(root, r.node.id, None))
                        .map(|pid| ItemTreeEvent::Selected(pid, SelectionMode::Replace))
                }
            }
            Key::Named(Named::ArrowRight) => {
                let r = &rows[cur];
                if !r.node.children.is_empty() && !r.node.is_expanded {
                    Some(ItemTreeEvent::Toggled(r.node.id))
                } else if r.node.is_expanded && !r.node.children.is_empty() {
                    Some(ItemTreeEvent::Selected(
                        r.node.children[0].id,
                        SelectionMode::Replace,
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl<T: Clone + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static> ItemTree<T> {
    /// Like `set_tree` but also re-runs any active search query.
    /// Use this variant when `T: Display` and search may be active.
    ///
    /// This is the method to call in most real applications, since
    /// search requires `T: Display`. For clarity, `set_tree` is
    /// available on all `ItemTree<T>` (with no `Display` bound) but
    /// will leave a stale search cache if called while search is
    /// active.
    pub fn set_tree_and_recompute_search(&mut self, root: ItemNode<T>) {
        self.set_tree(root);
        self.recompute_search_visibility();
    }

    // ---- accessors ----

    /// The most-recently-touched node id (`active_id`).
    pub fn active_id(&self) -> Option<NodeId> {
        self.active_id
    }

    /// The Shift-range anchor id.
    pub fn anchor_id(&self) -> Option<NodeId> {
        self.anchor_id
    }

    /// All currently selected node ids.
    pub fn selected_ids(&self) -> &[NodeId] {
        &self.selected_ids
    }

    /// Whether `id` is in the selection set.
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected_ids.contains(&id)
    }

    /// Whether a search query is active.
    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    /// The current search query as provided by the caller, or
    /// `None` when inactive.
    pub fn search_query(&self) -> Option<&str> {
        self.search.as_ref().map(|s| s.query.as_str())
    }

    /// Count of nodes that directly match the current search
    /// query (ancestors not counted). Returns `0` when inactive.
    pub fn search_match_count(&self) -> usize {
        self.search.as_ref().map_or(0, |s| s.match_count)
    }

    // ---- search ----

    /// Activate or update the incremental search query.
    ///
    /// The tree narrows its visible rows to basename-string matches
    /// plus every ancestor of every match, where "basename string"
    /// is `format!("{}", node.data).to_lowercase()`.
    ///
    /// An empty string clears the search (equivalent to
    /// [`clear_search`](Self::clear_search)).
    pub fn set_search_query(&mut self, query: impl Into<String>)
    where
        T: Display,
    {
        let q: String = query.into();
        if q.is_empty() {
            self.search = None;
            return;
        }
        self.search = Some(ItemSearchState::new(q));
        self.recompute_search_visibility();
    }

    /// Clear the active search query. No-op if already inactive.
    pub fn clear_search(&mut self) {
        self.search = None;
    }

    /// Recompute the search visibility cache. Called automatically
    /// by `set_tree`, `set_search_query`, and any operation that
    /// changes the node graph.
    fn recompute_search_visibility(&mut self)
    where
        T: Display,
    {
        let Some(state) = self.search.as_mut() else {
            return;
        };
        let Some(root) = &self.root else {
            state.visible_ids.clear();
            state.match_count = 0;
            return;
        };
        let mut visible = HashSet::new();
        let mut count = 0usize;
        walk_for_search(root, &state.query_lower.clone(), &mut visible, &mut count);
        state.visible_ids = visible;
        state.match_count = count;
    }

    // ---- view ----

    /// Produce the iced `Element` for this tree. Pass a mapper
    /// that wraps `ItemTreeEvent` into your application's
    /// `Message` type.
    ///
    /// ```ignore
    /// fn view(app: &App) -> Element<'_, Message> {
    ///     app.tree.view(Message::Tree)
    /// }
    /// ```
    pub fn view<'a, Message, F>(&'a self, on_event: F) -> Element<'a, Message>
    where
        Message: Clone + 'a,
        T: Display,
        F: Fn(ItemTreeEvent) -> Message + Copy + 'a,
    {
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        let theme = self.icon_theme.as_ref();

        if let Some(root) = &self.root {
            render_node(root, 0, &self.search, theme, on_event, &mut rows);
        }

        let list = column(rows).spacing(2).padding(4).width(Length::Fill);
        scrollable(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl<T: Clone + std::fmt::Debug + Send + Sync + 'static> Default for ItemTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---- view helpers ----

fn render_node<'a, T, Message, F>(
    node: &'a ItemNodeState<T>,
    depth: u32,
    search: &Option<ItemSearchState>,
    theme: &dyn IconTheme,
    on_event: F,
    out: &mut Vec<Element<'a, Message>>,
) where
    T: Clone + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    Message: Clone + 'a,
    F: Fn(ItemTreeEvent) -> Message + Copy + 'a,
{
    // Search filter: skip nodes not in the visible set.
    if search
        .as_ref()
        .is_some_and(|s| !s.visible_ids.contains(&node.id))
    {
        return;
    }

    out.push(render_row(node, depth, theme, on_event));

    // Descend: during search always descend; otherwise respect is_expanded.
    let descend = if search.is_some() {
        true
    } else {
        node.is_expanded
    };
    if descend {
        for child in &node.children {
            render_node(child, depth + 1, search, theme, on_event, out);
        }
    }
}

fn render_row<'a, T, Message, F>(
    node: &'a ItemNodeState<T>,
    depth: u32,
    theme: &dyn IconTheme,
    on_event: F,
) -> Element<'a, Message>
where
    T: Clone + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    Message: Clone + 'a,
    F: Fn(ItemTreeEvent) -> Message + Copy + 'a,
{
    let indent = (depth * 16) as u16;

    // Caret (only for branch nodes).
    let caret: Element<'a, Message> = if !node.children.is_empty() {
        let role = if node.is_expanded {
            IconRole::CaretDown
        } else {
            IconRole::CaretRight
        };
        let id = node.id;
        button(icon_render::<Message>(theme, role))
            .on_press(on_event(ItemTreeEvent::Toggled(id)))
            .padding(2)
            .style(iced::widget::button::text)
            .into()
    } else {
        // Invisible spacer so labels align with branch nodes.
        text(" ").size(14).into()
    };

    let label = text(format!("{}", node.data)).size(14);

    let id = node.id;
    let content = row![caret, label]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    let bg = if node.is_selected {
        iced::widget::container::background(iced::Background::Color(iced::Color::from_rgb(
            0.2, 0.5, 0.8,
        )))
    } else {
        iced::widget::container::background(iced::Background::Color(iced::Color::TRANSPARENT))
    };

    let left_pad = indent + 4;
    let row_widget = container(
        iced::widget::mouse_area(container(content).padding(iced::Padding {
            top: 2.0,
            right: 4.0,
            bottom: 2.0,
            left: left_pad as f32,
        }))
        .on_press(on_event(ItemTreeEvent::Selected(
            id,
            SelectionMode::Replace,
        ))),
    )
    .style(move |_| bg);

    row_widget.into()
}
