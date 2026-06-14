# Arrow: skill

The `vat` skill — prose implementation of VAT for zero-install agents, nested-repo detection, and the atomic claim loop.

## Status

**MAPPED** — first mapped 2026-06-14 (HEAD `ee8b0e6`). LLD (`docs/llds/skill.md`) and EARS spec (`docs/specs/skill-specs.md`) exist. The 5 binary-first specs (`SKILL-BIN-001..005`, added with vat-46x) are `[x]` — written in lockstep with the SKILL.md § Binary-first delegation implementation and verified coherent. The other 32 SKILL-* specs remain `[ ]` (not yet audited against the SKILL.md implementation); the SKILL.md implementation almost certainly satisfies them — they were specified after the skill was written — but an audit pass is needed to flip those markers.

## References

### HLD
- docs/high-level-design.md (§ Implementations, § Backlog as a nested repo, § Key design decisions #7)

### LLD
- docs/llds/skill.md

### EARS
- docs/specs/skill-specs.md (37 active specs: 5 verified; 32 unverified; 0 deferred)

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
| Fidelity and file boundary | SKILL-IMPL-001 to 002 | 0 | 0 | 2 |
| Nested-repo detection | SKILL-DETECT-001 to 004 | 0 | 0 | 4 |
| Atomicity guard | SKILL-GUARD-001 to 005 | 0 | 0 | 5 |
| Atomic claim loop | SKILL-LOOP-001 to 010 | 0 | 0 | 10 |
| Terminal preconditions | SKILL-TERM-001 to 008 | 0 | 0 | 8 |
| `config set project.id` (single push) | SKILL-CFG-001 to 003 | 0 | 0 | 3 |

**Summary:** 5 of 37 specs verified; 0 deferred; 32 unverified. The 5 verified are the binary-first specs (`SKILL-BIN-*`), written and verified with vat-46x. The 32 unverified gaps reflect lack of audit, not lack of implementation — the SKILL.md likely implements all of these. An audit pass should flip all or nearly all of those markers to `[x]`.

## Key Findings

0. **Binary-first delegation** (vat-46x) — The skill checks `command -v vat` before every operation. When the binary is installed it delegates (`vat <command>`) and reports the result; when absent it follows the prose procedures (its zero-install reason for existing). Delegation substitutes only the local file-mutation step — in a nested-repo backlog the skill still owns detection, the atomicity guard, the claim loop, terminal preconditions, and commit/push, because the binary is git-agnostic. Fidelity (`SKILL-IMPL-001`) makes the substitution safe: prose and binary produce byte-identical state. Specs `SKILL-BIN-001..005`.

1. **First-class implementation** — The skill is a permanent first-class implementation alongside the Rust binary (HLD § Implementations). It is not a stopgap. Its reason for existing is zero-install operation for remote coding agents who cannot run `cargo` or find a prebuilt binary in a fresh container.

2. **Nested-repo / atomic claim loop** — The skill's exclusive feature: when `backlog/.git` exists, every mutating command (`sync`, `start`, `block`, `unblock`, `done`) runs through the atomic first-push-wins claim loop (refresh → re-read → terminal-precondition check → mutate → commit → push). The binary is unaware of nested-repo backlogs and never runs git. `config set project.id` is excluded from the loop — it commits and pushes once and fails fast on rejection.

3. **Atomicity guard** — The `reset --hard` in the loop is destructive, so the skill evaluates the guard at entry: `backlog/` working tree must be clean AND `HEAD` must be an ancestor of `@{u}` (after a `git fetch`). If dirty or ahead, the skill falls back to a local edit only and reports the deferral. This prevents the loop from destroying uncommitted or unpushed work inherited from the user.

4. **All SKILL-* specs unverified** — The LLD and EARS spec were added alongside the final packaging work (ee8b0e6). Spec markers remain `[ ]` pending a dedicated audit pass tracing each SKILL-* requirement through the SKILL.md prose implementation.

## Work Required

### Must Fix
1. Audit all 32 SKILL-* specs against `.claude/skills/vat/SKILL.md` and flip markers to `[x]` as verified. Suggested order: SKILL-IMPL first (establishes fidelity contract), then SKILL-DETECT (detection logic), SKILL-GUARD (precondition), SKILL-LOOP (the loop body), SKILL-TERM (per-command terminal conditions), SKILL-CFG (single-push path).
