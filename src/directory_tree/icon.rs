//! Icon abstraction.
//!
//! The widget never calls lucide functions directly — it goes through
//! the [`Icon`] enum below. This keeps view code identical whether or
//! not the `icons` feature is enabled, and keeps the door open for a
//! future user-supplied icon theme (v0.5+).
//!
//! ## Feature flag behaviour
//!
//! * **With `icons`** — renders the real lucide TTF glyphs via the
//!   `lucide-icons` crate's `iced` helpers. The application must
//!   register the bundled font at startup:
//!   ```ignore
//!   iced::Settings {
//!       fonts: vec![iced_swdir_tree::LUCIDE_FONT_BYTES.into()],
//!       ..Default::default()
//!   }
//!   ```
//!   Forgetting to register the font is not fatal — the glyphs just
//!   render as tofu squares.
//!
//! * **Without `icons`** — falls back to short Unicode text symbols
//!   (📁 📂 📄 ⚠ ▸ ▾) that are available in any system font.

use iced::Element;

/// Semantic icon identifiers the widget needs.
///
/// Keeping this abstract — rather than hard-coding `icon_folder` into
/// view code — means swapping icon libraries in the future is a
/// `match` in one file.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Icon {
    /// A directory that is currently collapsed.
    FolderClosed,
    /// A directory that is currently expanded.
    FolderOpen,
    /// A regular file.
    File,
    /// A directory we could not list (permission denied, etc.).
    Error,
    /// The caret pointing right (collapsed indicator for folders).
    CaretRight,
    /// The caret pointing down (expanded indicator for folders).
    CaretDown,
}

/// Render `icon` as an `iced::Element`.
///
/// The produced element targets roughly the same visible size as a
/// 14-unit text glyph so row heights stay consistent across feature
/// configurations.
pub(crate) fn render<'a, Message: 'a>(icon: Icon) -> Element<'a, Message> {
    #[cfg(feature = "icons")]
    {
        render_lucide(icon)
    }
    #[cfg(not(feature = "icons"))]
    {
        render_text(icon)
    }
}

#[cfg(not(feature = "icons"))]
fn render_text<'a, Message: 'a>(icon: Icon) -> Element<'a, Message> {
    use iced::widget::text;
    let s = match icon {
        Icon::FolderClosed => "\u{1F4C1}", // 📁
        Icon::FolderOpen => "\u{1F4C2}",   // 📂
        Icon::File => "\u{1F4C4}",         // 📄
        Icon::Error => "\u{26A0}",         // ⚠
        Icon::CaretRight => "\u{25B8}",    // ▸
        Icon::CaretDown => "\u{25BE}",     // ▾
    };
    text(s).size(14).into()
}

#[cfg(feature = "icons")]
fn render_lucide<'a, Message: 'a>(icon: Icon) -> Element<'a, Message> {
    // lucide-icons' iced helpers return `iced::widget::Text<'a>`, which
    // keeps `.size()` / `.color()` available for us to tune here
    // without locking callers into our choice.
    use lucide_icons::iced::{
        icon_alert_circle, icon_chevron_down, icon_chevron_right, icon_file, icon_folder,
        icon_folder_open,
    };
    let size = 14.0_f32;
    match icon {
        Icon::FolderClosed => icon_folder().size(size).into(),
        Icon::FolderOpen => icon_folder_open().size(size).into(),
        Icon::File => icon_file().size(size).into(),
        Icon::Error => icon_alert_circle().size(size).into(),
        Icon::CaretRight => icon_chevron_right().size(size).into(),
        Icon::CaretDown => icon_chevron_down().size(size).into(),
    }
}
