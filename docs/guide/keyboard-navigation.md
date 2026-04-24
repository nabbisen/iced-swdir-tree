# Keyboard navigation

`DirectoryTree::handle_key(&Key, Modifiers) -> Option<DirectoryTreeEvent>`
translates a key press into the right event. The widget stays
focus-neutral — you decide when the tree has focus and subscribe
to the key stream yourself:

```rust,ignore
use iced::keyboard;

fn subscription(app: &App) -> iced::Subscription<Message> {
    keyboard::listen().map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } =>
            Message::TreeKey(key, modifiers),
        _ => Message::Noop,
    })
}

// ...in update:
Message::TreeKey(key, mods) => {
    if let Some(event) = self.tree.handle_key(&key, mods) {
        return self.tree.update(event).map(Message::Tree);
    }
    Task::none()
}
```

| Key | Behaviour |
|---|---|
| `↑` / `↓` | Move selection to previous / next visible row. |
| `Shift` + `↑` / `↓` | Extend the selected range toward the previous / next row. |
| `Home` / `End` | Jump to first / last visible row. |
| `Shift` + `Home` / `End` | Extend the range to the first / last row. |
| `Enter` | Toggle the selected directory (no-op on files). |
| `Space` / `Ctrl` + `Space` | Toggle the active path in or out of the selected set. |
| `←` | Collapse selected directory, or move to parent. |
| `→` | Expand selected directory, or move to first child. |
| `Esc` | Cancel an in-flight drag (only bound during drag, so apps can still use `Esc` for their own UI otherwise). |

See [`examples/keyboard_nav.rs`](../../examples/keyboard_nav.rs) for
a single-select navigation demo and
[`examples/multi_select.rs`](../../examples/multi_select.rs) for
multi-select with Shift/Ctrl-click.
