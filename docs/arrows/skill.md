# Arrow: skill

The `vat` skill — prose implementation of VAT for zero-install agents, nested-repo detection, and the atomic claim loop.

## Status

**OK** — first mapped 2026-06-14 (HEAD `ee8b0e6`); re-audited 2026-06-19 (HEAD `762d0fd`): all 37 SKILL-* specs verified (5 binary-first specs `SKILL-BIN-001..005` verified in vat-46x; 32 remaining specs verified by design-level audit of `.claude/skills/vat/SKILL.md`); no new drift introduced.

## References

### HLD
- docs/high-level-design.md (§ Implementations, § Backlog as a nested repo, § Key design decisions #7)

### LLD
- docs/llds/skill.md

### EARS
- docs/specs/skill-specs.md (37 active specs: 37 verified; 0 unverified; 0 deferred)

### Tests
- .claude/skills/vat/evals/evals.json (scenario evals covering SKILL-* behaviors, run with the skill-creator harness)

### Code
- .claude/skills/vat/SKILL.md — the prose implementation of all VAT commands for agents; nested-repo detection; atomic claim loop; atomicity guard

## Architecture

**Purpose:** A prose implementation of VAT that a coding agent follows directly — no binary to install, no `cargo`. Implements the same `FMT-*`, `SYNC-*`, `CMD-*` specs as the Rust binary; additionally implements `SKILL-*` specs covering: binary-first delegation (delegate to the installed `vat` binary, prose as fallback), fidelity/file-boundary contract, nested-repo detection (`backlog/.git`), the atomicity guard, the atomic first-push-wins claim loop, per-command terminal preconditions, and the `config set project.id` single-push path.

**Key Components:**
1. `.claude/skills/vat/SKILL.md` — the implementation artifact (a prose skill Claude Code follows)
2. `.claude/skills/vat/evals/evals.json` — scenario evals exercising SKILL-* behaviors
3. `docs/llds/skill.md` — design: atomic claim loop, atomicity guard, nested-repo detection rationale, decisions

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Binary-first delegation | SKILL-BIN-001 to 005 | 5 | 0 | 0 |
| Fidelity and file boundary | SKILL-IMPL-001 to 002 | 2 | 0 | 0 |
| Nested-repo detection | SKILL-DETECT-001 to 004 | 4 | 0 | 0 |
| Atomicity guard | SKILL-GUARD-001 to 005 | 5 | 0 | 0 |
| Atomic claim loop | SKILL-LOOP-001 to 010 | 10 | 0 | 0 |
| Terminal preconditions | SKILL-TERM-001 to 008 | 8 | 0 | 0 |
| `config set project.id` (single push) | SKILL-CFG-001 to 003 | 3 | 0 | 0 |

**Summary:** 37 of 37 specs verified; 0 deferred; 0 unverified.

## Key Findings

0. **Binary-first delegation** (vat-46x) — The skill checks `command -v vat` once, before performing any requested operation. When the binary is installed it delegates (`vat <command>`) and reports the result; when absent it follows the prose procedures (its zero-install reason for existing). Delegation substitutes only the local file-mutation step — in a nested-repo backlog the skill still owns detection, the atomicity guard, the claim loop, terminal preconditions, and commit/push, because the binary is git-agnostic. Fidelity (`SKILL-BIN-003`) makes the substitution safe: prose and binary produce byte-identical state. Specs `SKILL-BIN-001..005`.

1. **First-class implementation** — The skill is a permanent first-class implementation alongside the Rust binary (HLD § Implementations). It is not a stopgap. Its reason for existing is zero-install operation for remote coding agents who cannot run `cargo` or find a prebuilt binary in a fresh container.

2. **Nested-repo / atomic claim loop** — The skill's exclusive feature: when `backlog/.git` exists, every mutating command (`sync`, `start`, `block`, `unblock`, `done`) runs through the atomic first-push-wins claim loop (refresh → re-read → terminal-precondition check → mutate → commit → push). The binary is unaware of nested-repo backlogs and never runs git. `config set project.id` is excluded from the loop — it commits and pushes once and fails fast on rejection.

3. **Atomicity guard** — The `reset --hard` in the loop is destructive, so the skill evaluates the guard at entry: `backlog/` working tree must be clean AND `HEAD` must be an ancestor of `@{u}` (after a `git fetch`). If dirty or ahead, the skill falls back to a local edit only and reports the deferral. This prevents the loop from destroying uncommitted or unpushed work inherited from the user.

4. **All 37 SKILL-* specs verified** — The 5 binary-first specs (`SKILL-BIN-001..005`) were verified in vat-46x (written in lockstep with SKILL.md § Binary-first delegation). The remaining 32 specs were verified by design-level audit at `762d0fd` (2026-06-19): SKILL-IMPL to the file-boundary list and procedures; SKILL-DETECT to the detection section (lines 166–171); SKILL-GUARD to the atomicity guard section (lines 173–188); SKILL-LOOP to the loop body (lines 192–215); SKILL-TERM to the terminal preconditions table (lines 219–230); SKILL-CFG to the single-push path (lines 244–246). Note: SKILL-IMPL-001's byte-identical claim is verified by design-level audit only (both skill and binary implement the same `FMT-*`/`SYNC-*`/`CMD-*` specs) — not by a live execution comparison. A future eval fixture for runtime verification is noted as a deferred item in `evals.json`.
