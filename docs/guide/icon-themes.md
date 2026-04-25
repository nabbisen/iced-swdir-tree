# Icon themes

The widget draws every folder, file, caret, and error indicator
through an [`IconTheme`](https://docs.rs/iced-swdir-tree/latest/iced_swdir_tree/trait.IconTheme.html)
trait. The crate ships two stock themes, picks the right one for
your feature configuration automatically, and accepts your own
theme via `with_icon_theme`.

## The stock themes

### `UnicodeTheme` (always available)

Renders short Unicode symbols (📁 📂 📄 ⚠ ▸ ▾) in the default
system font. No font registration required; nothing to pull in.
This is the default theme when the `icons` feature is disabled.

### `LucideTheme` (`icons` feature)

Renders real [lucide](https://lucide.dev) vector glyphs via the
bundled `LUCIDE_FONT_BYTES` TTF. Register the font at iced
startup so the codepoints actually draw:

```rust,ignore
iced::application(App::new, App::update, App::view)
    .font(iced_swdir_tree::LUCIDE_FONT_BYTES)
    .run()
```

Without the font registered, the glyphs render as tofu squares
— the widget still works (selection, drag-drop, etc.), just
with wrong-looking icons. This is the default theme when the
`icons` feature is enabled.

## Implementing a custom theme

Three pieces: the trait, the role enum, the spec struct.

```rust,ignore
use std::sync::Arc;
use iced_swdir_tree::{
    DirectoryTree, IconRole, IconSpec, IconTheme,
};

#[derive(Debug)]
struct MaterialTheme {
    font: iced::Font,
}

impl IconTheme for MaterialTheme {
    fn glyph(&self, role: IconRole) -> IconSpec {
        // Codepoints come from the Material Icons font. A real
        // implementation would probably use a `match` against
        // named codepoints from a helper crate.
        let codepoint: char = match role {
            IconRole::FolderClosed => '\u{E2C7}',   // folder
            IconRole::FolderOpen   => '\u{E2C8}',   // folder_open
            IconRole::File         => '\u{E873}',   // description
            IconRole::Error        => '\u{E000}',   // error
            IconRole::CaretRight   => '\u{E315}',   // chevron_right
            IconRole::CaretDown    => '\u{E313}',   // expand_more
            // IconRole is #[non_exhaustive] — always add a
            // fallback arm for future-proofing.
            _ => '?',
        };
        IconSpec::new(codepoint.to_string())
            .with_font(self.font)
            .with_size(16.0)
    }
}

let tree = DirectoryTree::new(".".into())
    .with_icon_theme(Arc::new(MaterialTheme {
        font: iced::Font::with_name("Material Icons"),
    }));
```

Your app also needs to register the Material Icons font with
iced — same pattern as `LUCIDE_FONT_BYTES`, but with your own
font bytes.

## What goes in `IconSpec`?

- **`glyph: Cow<'static, str>`** — the text to render. Usually
  a single char, but ligatures, emoji sequences, and
  multi-character labels (`"DIR"`) all work.
- **`font: Option<iced::Font>`** — the font to render in.
  `None` means the iced default; set a specific font when you
  need an icon-font's codepoints.
- **`size: Option<f32>`** — the point size. `None` means the
  widget picks (currently 14).

Public fields so `const`-style themes are easy; builder methods
(`new`/`with_font`/`with_size`) for ergonomics. The struct is
**not** `#[non_exhaustive]`, so adding a field in a future
major release would be breaking — treat v0.7 as the final
shape.

## `IconRole` is `#[non_exhaustive]`

New roles may be added in future minor releases (candidates:
`Symlink`, `Hidden`, `Loading`). Your theme's `match` must
include a `_ =>` fallback arm, or it will fail to compile
against the next minor release.

Stock themes are allowed to match exhaustively because they
live in the same crate — the `#[non_exhaustive]` annotation
only restricts external code.

## When to turn off the `icons` feature

The `icons` feature pulls in the ~300KB lucide TTF plus the
`lucide-icons` crate's per-icon wrappers. If you've plugged in
your own theme, none of that is used at runtime. Turn the
feature off to shrink your binary:

```toml
[dependencies]
iced-swdir-tree = { version = "0.7", default-features = false }
```

That leaves you with `UnicodeTheme` available as a stock
fallback, `IconTheme` / `IconRole` / `IconSpec` for your custom
theme, and no lucide bytes at all.

## See also

- [`examples/icon_theme.rs`](../../examples/icon_theme.rs) — a
  three-way theme switcher (Unicode / Label / Ascii) that
  demonstrates the full trait surface.
- [Configuration](configuration.md) — where `with_icon_theme`
  fits among the other builder methods.
