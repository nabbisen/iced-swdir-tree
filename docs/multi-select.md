# Multi-select

The widget keeps a full selected set, a "most-recent-action"
active path, and an anchor path for Shift-range extension.
`SelectionMode` — exported from the crate root — controls how
each click composes:

| Mode | Effect |
|---|---|
| `Replace` | Clear the set; the new path becomes the only selection. Updates both active and anchor. |
| `Toggle`  | Add if absent, remove if present. Updates both active and anchor. |
| `ExtendRange` | Replace the set with the visible rows between anchor and target, inclusive. Only active moves. Falls back to `Replace` if no anchor is set. |

## View-level click behaviour

iced 0.14's `button::on_press` cannot observe modifier keys, so
the widget's built-in view always emits `SelectionMode::Replace`
on click. Applications that want real multi-select track
modifier state separately and rewrite the event in their own
update handler:

```rust,ignore
use iced::keyboard::{self, Modifiers};
use iced_swdir_tree::{DirectoryTreeEvent, SelectionMode};

// In your update:
Message::Tree(DirectoryTreeEvent::Selected(path, is_dir, _)) => {
    let mode = SelectionMode::from_modifiers(self.modifiers);
    let event = DirectoryTreeEvent::Selected(path, is_dir, mode);
    self.tree.update(event).map(Message::Tree)
}
Message::ModifiersChanged(m) => {
    self.modifiers = m;
    Task::none()
}

// In your subscription:
fn subscription(app: &App) -> iced::Subscription<Message> {
    keyboard::listen().map(|event| match event {
        keyboard::Event::ModifiersChanged(m) => Message::ModifiersChanged(m),
        keyboard::Event::KeyPressed { key, modifiers, .. } =>
            Message::TreeKey(key, modifiers),
        _ => /* ... */,
    })
}
```

See [`examples/multi_select.rs`](../examples/multi_select.rs) for
a complete working app with a live selection-count status bar.
