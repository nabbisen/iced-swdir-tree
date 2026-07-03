# RFC process

Design decisions for this crate are recorded as RFCs under the
[`rfcs/`](../../../rfcs/) directory, following the lifecycle policy in
`000-rfc-lifecycle-policy.md`.

## Lifecycle

An RFC lives in exactly one folder, and the **folder is the source of
truth** for its state:

- `rfcs/proposed/` — open for review; implementation should not start
  yet.
- `rfcs/done/` — implemented and shipped; a historical record of the
  decision and the alternatives weighed.
- `rfcs/archive/` — withdrawn or superseded, kept with a one-line
  reason so the same idea is not re-proposed from scratch.

Each RFC also carries a `Status` field mirroring its folder. RFCs are
never deleted and never renumbered; a withdrawn number stays withdrawn.

## Current RFCs

See [`rfcs/README.md`](../../../rfcs/README.md) for the live index. As
of this writing:

- **RFC 001** — Generic item tree (`ItemTree<T>`) — *done*, v0.8.0.
- **RFC 002** — Drag-and-drop for `ItemTree<T>` — *done*, v0.9.0.
- **RFC 003** — Extract a framework-agnostic `swdir-tree-core` —
  *withdrawn*. The resolved position is to share the **design**
  (these documents), not a code crate; each UI framework implements
  the spec idiomatically. See the archived RFC and
  [Porting to other frameworks](porting-to-dioxus.md) for the full
  rationale.

## Where design and code meet

The [feature specifications](feature-specs.md) are the behavioural
oracle: tests validate the numbered clauses there, not merely the
written code. When a change alters behaviour, update the specification
first, then the code and tests. See
[Development & testing](development.md) for the local workflow.
