---
version: 1
---

# VAT implementation backlog

Tasks to bring VAT from spec to a working `cargo install`-able binary. Roughly ordered bottom-up: format primitives, then commands, then packaging.

- [vat-f1w] Markdown parser: parsed region into preamble + task entries (see ./items/vat-f1w.md)
- [vat-g5y] [blocked-by:vat-f1w] Bullet line tokenizer (markers + title) (see ./items/vat-g5y.md)
- [vat-j3z] [blocked-by:vat-g5y] Bullet line serializer (canonical order) (see ./items/vat-j3z.md)
- [vat-p7d] Version check cross-cutting helper (see ./items/vat-p7d.md)
- [vat-q2e] [blocked-by:vat-r6f] `vat init` command (see ./items/vat-q2e.md)
- [vat-r6f] README template (baked into binary) (see ./items/vat-r6f.md)
- [vat-s9g] `vat sync` command — ID assignment (see ./items/vat-s9g.md)
- [vat-t1h] `vat sync` command — notes extraction (see ./items/vat-t1h.md)
- [vat-v3k] [blocked-by:vat-j3z] `vat sync` command — marker normalization and write (see ./items/vat-v3k.md)
- [vat-w5m] [blocked-by:vat-j3z] `vat start <id>` command (see ./items/vat-w5m.md)
- [vat-x8n] [blocked-by:vat-j3z] `vat block <id> <blocker-id>` command (see ./items/vat-x8n.md)
- [vat-y2p] [blocked-by:vat-j3z] `vat unblock <id>` command (see ./items/vat-y2p.md)
- [vat-z4q] [blocked-by:vat-j3z] `vat done <id>` command (see ./items/vat-z4q.md)
- [vat-b6r] `vat config get/set` commands (see ./items/vat-b6r.md)
- [vat-c9s] Exit codes wiring (see ./items/vat-c9s.md)
- [vat-d3t] [blocked-by:vat-v3k] Snapshot / golden-file tests for sync (see ./items/vat-d3t.md)
- [vat-f7v] [blocked-by:vat-z4q] Snapshot tests for the other commands (see ./items/vat-f7v.md)
- [vat-g4w] [blocked-by:vat-z4q] End-to-end CLI tests (see ./items/vat-g4w.md)
- [vat-h2y] Project README at repo root (see ./items/vat-h2y.md)
- [vat-j5z] [blocked-by:vat-g4w] Release packaging (see ./items/vat-j5z.md)
- [vat-k1b] [blocked-by:vat-q2e] Shell completions (see ./items/vat-k1b.md)
- [vat-n3x] Switch test runner to `cargo-nextest` (see ./items/vat-n3x.md)

---

Anything below this line is freeform notes and is not parsed by VAT.

