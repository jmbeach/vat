---
version: 1
---

# VAT implementation backlog

Tasks to bring VAT from spec to a working `cargo install`-able binary. Roughly ordered bottom-up: format primitives, then commands, then packaging.

- [vat-k7p] Set up Rust project scaffolding
- [vat-q3m] Crockford base32 utilities
- [vat-h8x] Project config loader (`backlog/vat.toml`)
- [vat-r2n] User config loader (`~/.config/vat/config.toml`)
- [vat-v4j] Tombstone file read/write (`backlog/.used-ids`)
- [vat-b9s] Markdown parser: frontmatter
- [vat-d6t] [blocked-by:vat-b9s] Markdown parser: body region split
- [vat-f1w] [blocked-by:vat-d6t] Markdown parser: parsed region into preamble + task entries
- [vat-g5y] [blocked-by:vat-f1w] Bullet line tokenizer (markers + title)
- [vat-j3z] [blocked-by:vat-g5y] Bullet line serializer (canonical order)
- [vat-m8b] Line-ending normalization
- [vat-n4c] Item file read/write/append (`backlog/items/<id>.md`)
- [vat-p7d] Version check cross-cutting helper
- [vat-q2e] [blocked-by:vat-r6f] `vat init` command
- [vat-r6f] README template (baked into binary)
- [vat-s9g] [blocked-by:vat-q3m] `vat sync` command — ID assignment
- [vat-t1h] [blocked-by:vat-n4c] `vat sync` command — notes extraction
- [vat-v3k] [blocked-by:vat-j3z] `vat sync` command — marker normalization and write
- [vat-w5m] [blocked-by:vat-j3z] `vat start <id>` command
- [vat-x8n] [blocked-by:vat-j3z] `vat block <id> <blocker-id>` command
- [vat-y2p] [blocked-by:vat-j3z] `vat unblock <id>` command
- [vat-z4q] [blocked-by:vat-j3z] `vat done <id>` command
- [vat-b6r] [blocked-by:vat-h8x] `vat config get/set` commands
- [vat-c9s] Exit codes wiring
- [vat-d3t] [blocked-by:vat-v3k] Snapshot / golden-file tests for sync
- [vat-f7v] [blocked-by:vat-z4q] Snapshot tests for the other commands
- [vat-g4w] [blocked-by:vat-z4q] End-to-end CLI tests
- [vat-h2y] Project README at repo root
- [vat-j5z] [blocked-by:vat-g4w] Release packaging
- [vat-k1b] [blocked-by:vat-q2e] Shell completions

---

Anything below this line is freeform notes and is not parsed by VAT.

## Open questions to revisit

- Do we want `vat list` / `vat ls` to print a summary of bullets and their states? Pure read-only, no file mutation. Could be useful for agents.
- Should `vat done` accept multiple IDs (`vat done foo-7k2 foo-9hf`)?
- Telemetry / first-run nudges — probably none for v1.
