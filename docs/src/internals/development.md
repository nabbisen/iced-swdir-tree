# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The v0.9.x release gate covers 213 tests (123 unit, 83 integration,
7 doctests; 15 additional doctests are ignored because they require
a live iced runtime). The suites cover every filter mode,
expand/collapse round-trips, both single- and multi-select
(Replace / Toggle / ExtendRange), stale-result rejection,
permission-denied, keyboard bindings, custom executor integration,
directory drag-and-drop, prefetch, incremental search, icon themes,
generic item-tree behaviour, and `ItemTree` reorder/nest
drag-and-drop. See the
[CHANGELOG](../../../CHANGELOG.md) for the per-release breakdown and
[ROADMAP](../../../ROADMAP.md) for what's next.
