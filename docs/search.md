# Incremental search

Wire an `iced::widget::text_input` into the tree via
`DirectoryTree::set_search_query`. The widget narrows its
visible rows to basename-substring matches (case-insensitive)
plus every ancestor of every match — so users see where matches
live in the tree, not just isolated filenames.

```rust,ignore
#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    // ...
}

// In update:
Message::SearchChanged(q) => {
    self.tree.set_search_query(q);
    Task::none()
}

// In view:
text_input("Filter...", &self.query).on_input(Message::SearchChanged)
```

Four accessors drive a "N matches" status line or a clear
button:

```rust,ignore
tree.is_searching();             // bool
tree.search_query();             // Option<&str> (original casing)
tree.search_match_count();       // usize — excludes ancestor rows
tree.clear_search();             // drop the query
```

## Semantics

- **Case-insensitive basename substring match.** The path
  components ("/src/…") don't match — only the filename at each
  level does.
- **Empty string = cleared search.** There is no "searching for
  an empty string" state.
- **Already-loaded nodes only.** Matches inside unloaded folders
  don't appear until the folder loads. Combine with
  [`with_prefetch_limit(N)`](prefetch.md) for broader coverage
  without the user expanding everything manually.
- **Sees through collapsed-but-loaded folders.** A match deep
  inside a collapsed subtree still shows up; ancestors render as
  if expanded.
- **Selection survives.** Hidden-by-search selections are
  preserved and reappear when the query clears.

## Known limitation

Clicking a folder during search does **not** escape the filter.
The view stays narrowed to matches plus ancestors. To explore
outside the match set, clear the search first. See
[`examples/search.rs`](../examples/search.rs) for a complete
working app with text-input, counter, and expand-all button.
