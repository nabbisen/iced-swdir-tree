//! Render a [`DirectoryTree`] as an `iced::Element`.
//!
//! The layout is a vertical scrollable column of rows; each row is a
//! horizontal strip of indentation, caret, icon, and a button that
//! emits the row's click event. The view delegates icon selection to
//! the [`icon`](super::icon) module so the `icons` feature toggle never
//! leaks into view logic.
//!
//! ## Virtualization
//!
//! Only nodes in collapsed ancestors are skipped (the column shrinks
//! when they're closed). For very large loaded trees, iced's
//! `Scrollable` clips off-screen rows at render time — see
//! `iced::widget::scrollable` — so the cost of keeping them in the
//! element tree is limited to the layout pass. This is the best we
//! can do in iced 0.14 without a custom low-level widget, and it
//! matches the spec's "avoid rendering nodes outside the visible area
//! whenever possible" language.

use std::path::Path;

use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};

use super::DirectoryTree;
use super::icon::{Icon, render as icon_render};
use super::message::DirectoryTreeEvent;
use super::node::TreeNode;
use super::selection::SelectionMode;

/// Per-indent-level horizontal padding in logical pixels.
const INDENT_STEP: f32 = 16.0;
/// Horizontal gap between the caret, the icon, and the label, in
/// logical pixels. iced 0.14's `.spacing()` takes `impl Into<Pixels>`;
/// `f32` implements that conversion.
const INTRA_ROW_GAP: f32 = 6.0;

impl DirectoryTree {
    /// Build an `iced::Element` that renders this tree.
    ///
    /// `on_event` is the closure that maps the widget's internal
    /// [`DirectoryTreeEvent`]s into the parent application's own
    /// message type. See the crate-level docs for a worked example.
    pub fn view<'a, Message, F>(&'a self, on_event: F) -> Element<'a, Message>
    where
        Message: Clone + 'a,
        F: Fn(DirectoryTreeEvent) -> Message + Copy + 'a,
    {
        // Recurse over the tree and collect rows into a single column
        // inside a scrollable. `column` accepts an iterator, but we
        // build a Vec explicitly because the recursion depth can
        // exceed what inference wants to handle for a chained chain.
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        render_node(&self.root, 0, on_event, &mut rows);

        let list = column(rows).spacing(2).padding(4).width(Length::Fill);

        scrollable(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Render a single node and its descendants (if expanded) into `out`.
fn render_node<'a, Message, F>(
    node: &'a TreeNode,
    depth: u32,
    on_event: F,
    out: &mut Vec<Element<'a, Message>>,
) where
    Message: Clone + 'a,
    F: Fn(DirectoryTreeEvent) -> Message + Copy + 'a,
{
    out.push(render_row(node, depth, on_event));

    if node.is_dir && node.is_expanded && node.is_loaded {
        for child in &node.children {
            render_node(child, depth + 1, on_event, out);
        }
    }
}

/// Render a single row of the tree.
fn render_row<'a, Message, F>(node: &'a TreeNode, depth: u32, on_event: F) -> Element<'a, Message>
where
    Message: Clone + 'a,
    F: Fn(DirectoryTreeEvent) -> Message + Copy + 'a,
{
    // Visible label: the entry's file name, with a fallback to the
    // full path for the root (whose file_name() may be None, e.g.
    // `/` on Unix or `C:\` on Windows).
    let label_str: String = match node.path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => node.path.display().to_string(),
    };

    // The folder/file icon.
    let type_icon: Element<'a, Message> = if node.error.is_some() {
        icon_render::<Message>(Icon::Error)
    } else if node.is_dir {
        if node.is_expanded {
            icon_render::<Message>(Icon::FolderOpen)
        } else {
            icon_render::<Message>(Icon::FolderClosed)
        }
    } else {
        icon_render::<Message>(Icon::File)
    };

    // The label itself. Permission-denied rows render in a muted
    // foreground so the user sees at a glance that the node is
    // unreadable rather than merely empty. iced 0.14 doesn't expose
    // a single "dimmed" helper, so we set a literal mid-grey that
    // works acceptably on both light and dark themes.
    let label_widget = {
        let t = text(label_str).size(14);
        if node.error.is_some() {
            t.color(iced::Color::from_rgb(0.55, 0.55, 0.55))
        } else {
            t
        }
    };

    // --- Caret (the fold/unfold affordance) ----------------------
    //
    // We split the row into two click targets *side by side* rather
    // than nesting a caret button inside a selection button: iced's
    // button-inside-button hit-testing is undefined and can swallow
    // the inner press. The caret handles Toggled; the rest of the
    // row (icon + label inside a second button) handles Selected.
    let caret: Element<'a, Message> = if node.is_dir {
        let caret_icon = if node.is_expanded {
            Icon::CaretDown
        } else {
            Icon::CaretRight
        };
        let path = node.path.clone();
        button(icon_render::<Message>(caret_icon))
            .padding(2)
            .style(button::text)
            .on_press(on_event(DirectoryTreeEvent::Toggled(path)))
            .into()
    } else {
        // Files: fixed-size placeholder so the icon column aligns
        // with the directory rows above and below.
        Space::new()
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .into()
    };

    // --- Selection body (icon + label) ---------------------------
    let selection_body = row![
        type_icon,
        Space::new().width(Length::Fixed(4.0)),
        label_widget,
    ]
    .spacing(INTRA_ROW_GAP)
    .align_y(Alignment::Center);

    let select_button = {
        let path = node.path.clone();
        let is_dir = node.is_dir;
        button(selection_body)
            .width(Length::Fill)
            .padding(2)
            .style(if node.is_selected {
                button::primary
            } else {
                button::text
            })
            // Plain click always emits Replace — iced 0.14's button
            // `on_press` can't observe modifier keys. Apps that want
            // multi-select intercept this event and rewrite the
            // mode based on modifier state they track via the
            // keyboard subscription. See `examples/multi_select.rs`.
            .on_press(on_event(DirectoryTreeEvent::Selected(
                path,
                is_dir,
                SelectionMode::Replace,
            )))
    };

    // Left indent. Using a Space rather than padding so the selection
    // highlight runs the full visible row width — padding would
    // shrink the highlight by the indent amount.
    let indent_px = INDENT_STEP * depth as f32;
    let indent = Space::new().width(Length::Fixed(indent_px));

    container(
        row![indent, caret, select_button]
            .spacing(INTRA_ROW_GAP)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .into()
}

/// (Kept for future debugging.) Format a path for display in a row's
/// tooltip.
#[allow(dead_code)]
fn display_path(path: &Path) -> String {
    path.display().to_string()
}
