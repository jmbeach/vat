- [red] [vat-r2n] User config loader (`~/.config/vat/config.toml`) (see backlog/items/vat-r2n.md)
- [orange] [vat-v4j] Tombstone file read/write (`backlog/.used-ids`) (see backlog/items/vat-v4j.md)
- [yellow] [vat-d6t] Markdown parser: body region split (see backlog/items/vat-d6t.md)
- [green] [vat-m8b] Line-ending normalization (see backlog/items/vat-m8b.md)
- [blue] [vat-n4c] Item file read/write/append (`backlog/items/<id>.md`) (see backlog/items/vat-n4c.md)
---
# GBIV.md

Add features above the `---` line. Each feature starts with `- ` and an optional `[color]` tag.

Example:

- [red] My urgent feature
  A note about this feature
- [green] A less urgent feature
- An untagged backlog item

Supported tags match ROYGBIV colors: red, orange, yellow, green, blue, indigo, violet.
Untagged items appear with a dim `backlog` label.
Everything below `---` is ignored by gbiv.
