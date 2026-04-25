# Development

## Testing

```sh
cargo test --all-features
cargo clippy --all-targets --all-features
```

The crate ships 154 tests (88 unit + 65 integration + 1 doctest)
covering every filter mode, expand/collapse round-trips, both
single- and multi-select (Replace / Toggle / ExtendRange, with
survival across runtime filter changes), stale-result rejection,
permission-denied, keyboard bindings (including Shift-range,
`Space`-toggle, and `Escape` to cancel drags), custom executor
integration, the full drag-and-drop state machine
(is-valid-target rules, Pressed/Entered/Exited/Released/Cancelled
transitions, multi-item drag, descendant rejection), v0.5
prefetch (select-targets edge cases, cascade prevention,
limit/max_depth interaction, instant-fast-path on prefetched
click), v0.6 incremental search (case-insensitive basename
matching, multi-subtree, empty-clears, case-insensitivity,
selection preservation, filter-change-re-runs, collapsed-subtree
descent, on-loaded-recomputes), v0.6.1 prefetch safety valve
(default-skips-.git/node_modules/target, custom-replaces-default,
empty-disables, exact-basename-not-substring, ASCII-case-
insensitive, user-click-still-scans-skipped), and v0.7 icon
themes (UnicodeTheme/LucideTheme stock glyphs, IconSpec builder,
custom theme pluggability, object-safety of `Arc<dyn IconTheme>`,
and view-layer dispatch via a CountingTheme that verifies the
widget actually consults the theme). See the
[CHANGELOG](../../CHANGELOG.md) for the per-release breakdown and
[ROADMAP](../../ROADMAP.md) for what's next.
