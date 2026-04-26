---
id: vat-p7d
---

Single function called at the top of every read-path command. Reads frontmatter, compares `version` to a const `SUPPORTED_MAJOR`. Aborts with the standard error message if too new. CMD-CC-001, FMT-FM-002.
