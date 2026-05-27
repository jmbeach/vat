---
name: fleet-snapshot
description: Aggregate the current state of every roy-managed color pane into a single local markdown file, plus a self-contained context appendix (HLD, LLDs, EARS specs, in-flight task notes, referenced Rust source) so the file can be pasted into a fresh chat (e.g. claude.ai) for verbal discussion. Use when the user asks for a "fleet snapshot", wants to "roll up" what every color is doing, wants questions or PR links collected in one place, wants to "hand off to claude.ai", or says things like "what's everyone waiting on" / "give me one doc with all the open questions" / "snapshot the fleet".
argument-hint: (no args)
allowed-tools: Bash, Read
---

# fleet-snapshot

Two-step flow: **judgment** (you, reading roy output) then **aggregation** (a deterministic Python script).

## Step 1 — Read each pane and judge state

Run the aggregator script first. It calls `roy status` for you and returns sanitized tails per color, plus building the snapshot file from whatever artifacts are already on disk:

```bash
python3 .claude/skills/fleet-snapshot/scripts/build_snapshot.py
```

The script prints JSON of the form:

```json
{
  "snapshot": ".claude-cache/fleet-snapshot.md",
  "panes": [
    {"color": "red", "pane_status": "ok", "task_id": "vat-r2n",
     "has_questions_doc": true, "pr_url": null,
     "tail": "...last ~40 non-chrome lines of the pane..."}
  ]
}
```

Read each pane's `tail` and judge its state yourself. The key distinction is **what kind of thing the pane is asking the user for**:

### Nudge with a questions-doc message: pane is asking the user to make a DECISION

The pane is at a STOP / Phase boundary and wants the user to pick / confirm / approve / decide something. Examples from real tails:

- `STOP for Phase 2/3/4 review. Please confirm: LLD update looks right, new specs FMT-USR-003/004 look right, ...` → red wants design decisions confirmed.
- `STOP for review (Phase 2 + 3 cascade). ... Approve and I'll proceed to Phase 5.` → orange wants LLD/spec changes approved.
- `Still waiting on Q1–Q4 before moving to Phase 5.` → yellow has open numbered questions.
- `Phase 2 STOP. ... OK to proceed to Phase 3?` → green wants approval of a Phase 2 edit before continuing.

These all warrant a nudge. The pane should write the decision(s) it's asking about to `.claude-cache/<task-id>-questions.md` with options + pros/cons so the user can review them as a clean doc rather than parsing terminal output.

### Do NOT nudge: pane is asking the user to REVIEW WORK PRODUCT

The pane finished writing real artifacts (tests, code, a PR) and wants the user to go look at them directly. Examples:

- `Phase 5 complete. 27 tests, all compile, all fail with unimplemented!. Approve to move to Phase 6 (code)?` → blue wants the user to actually read the tests it wrote, not a meta-doc about them.
- `PR opened: https://...` / `Implementation complete, ready for review.` → user should look at the code/PR.
- A pane in the middle of producing output / between phases. → not waiting on anything.
- A pane quietly idle at the prompt with no STOP signal. → not waiting on anything.

A nudge here is noise: the user wants to **see the work**, not a summary of "questions" about the work.

### Ignore text after `❯` in pane tails

Text that appears after the `❯` prompt in a pane tail is almost always Claude's auto-suggestion (a ghosted command preview), not user-typed input awaiting Enter. **Do not** tell the user "you have unsent text in pane X, switch to tmux and hit Enter." Treat the prompt line as empty.

### Edge cases

- **Questions doc already exists.** Still nudge if the pane is at a fresh STOP. The existing doc was for an earlier decision; the current STOP is a new one and the pane should update or append.
- **Yellow-style "still waiting on existing questions".** Nudge so the pane can update the doc if it has new options or context. The doc-already-exists case is not a free pass to skip.
- **Ambiguous (could be design decision OR review).** Ask the user before sending.

## Step 2 — Send nudges only where appropriate

For each pane you judged actually needs prodding, send a nudge via roy:

**Decisions-to-doc nudge** (when the pane is asking for design decisions / confirmations / approvals):

```bash
roy send <color> "You're at a STOP / decision point. Please capture every decision you're asking me to make right now — approvals, confirmations, choices — as questions in .claude-cache/<task-id>-questions.md, each with 2-4 options and pros/cons. If a questions doc already exists, update or append so it reflects the current decision(s) on the table. Then wait for my input. Don't pick an option yourself."
```

**PR nudge** (when the user explicitly asks for a PR, or a pane reports a clearly code-complete state with no pending design decisions — rare; usually skip unless asked):

```bash
roy send <color> "Commit any uncommitted changes on your current branch, push the branch to origin, then run \`gh pr create --draft --fill\` and write the resulting PR URL (just the URL on one line) to .claude-cache/<task-id>-pr.url. If a draft PR already exists for this branch, just write its URL to that file."
```

Run multiple nudges as parallel Bash calls in one message.

**If you're unsure whether a pane needs a nudge, ask the user before sending.** A wrong nudge wastes pane attention and can derail mid-walk work.

## Step 3 — Report

Tell the user:
- The snapshot path (`.claude-cache/fleet-snapshot.md`).
- One short sentence per pane covering your judgment (e.g. "red: idle, questions doc on disk, no nudge needed").
- Which colors got nudged and why.
- If you nudged anyone: re-run the skill in a minute to pick up freshly-written files.

## Handoff to claude.ai

The generated file starts with a `# Handoff prompt — read this first` section that briefs a fresh chat on what the file is, how it's organized, and how to engage. So a user dropping the file into claude.ai (or running `pbcopy < .claude-cache/fleet-snapshot.md` and pasting) gets the briefing for free — no additional cover message needed.

If the user asks "how do I hand this off", tell them: drag the file into a new claude.ai chat (or paste it). The opening prompt is already inside.

## When NOT to invoke

- The user only wants to know *one* pane's status → use the `roy` skill directly (`roy get <color>`).
- The user wants to send a custom message to a pane → use `roy send` directly.
- `roy status` shows no `ok` panes → tell the user the daemon may not be running or no panes are started; do not run further.

## Files this skill touches

- Reads: `roy status` output; per-color `~/code/sandbox/vat/<color>/vat/.claude-cache/*-questions.md` and `*-pr.url`; `docs/`, `src/`, `backlog/items/` for the context appendix.
- Writes: `.claude-cache/fleet-snapshot.md` in the current working directory.
- Sends via roy: nudge messages, only to panes you've judged need them.

Nothing else.
