---
id: vat-c4d
---

A project-scoped skill under `.claude/skills/` that performs the same
operations the eventual Rust binary will: read/write `backlog.md`,
assign IDs, normalize markers, extract notes into `items/<id>.md`,
append to `.used-ids`. Claude uses this skill to manage this very
backlog while we build the real `vat`. Spec is the same as the binary
(HLD + LLDs + EARS in `docs/`), just executed by Claude instead of
Rust. Retire the skill once the binary is `cargo install`-able and
passes parity checks against the same fixtures.
