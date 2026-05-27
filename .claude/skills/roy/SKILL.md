---
name: roy
description: Orchestrate Claude Code sessions running in `gbiv` color worktrees via the `roy` CLI (formerly `gbork`). Use when the user wants to start the roy daemon, check status across colors, peek at a color's pane output, or send a prompt into one of the color sessions. Triggers include "roy status", "what's red doing", "send to green", "tail blue", "start the roy daemon", or any reference to roy/gbork orchestration.
argument-hint: <subcommand> [args...]
allowed-tools: Bash, Read
---

# roy skill

`roy` orchestrates Claude Code sessions, one per ROYGBIV color, each running in its own git worktree paired to a bullet in `gbiv.md`. Each color has a tmux pane; `roy` lets you watch and drive those panes from the outside.

## Subcommands

| Command | Purpose |
|---|---|
| `roy start [--session-name <name>]` | Run the orchestrator daemon in the foreground. Long-running. Ctrl+C to stop. |
| `roy status [--lines N] [--json]` | One-shot status across all colors. Default `--lines 50`. |
| `roy get <color> [--lines N] [--json]` | Print captured pane output for one color. Default `--lines 200`. |
| `roy send <color> <text>` | Send literal text + Enter into the named color's Claude pane. |

`<color>` is one of `red orange yellow green blue indigo violet`.

The `--bind` flag on `start` is reserved and ignored in v1 — don't pass it.

## When to invoke this skill

- "roy status" / "show me what each color is doing" → `roy status` (use `--json` if the user wants structured output, otherwise plain).
- "what's red doing" / "tail green" / "show the last 500 lines from blue" → `roy get <color> [--lines N]`.
- "send 'continue' to violet" / "tell yellow to run the tests" → `roy send <color> "<text>"`.
- "start roy" / "run the daemon" → `roy start`. **Always run this in the background** (long-lived), and tell the user it's running and how to stop it.
- Any reference to `gbork` — that's the old name; the binary is `roy` now. Use `roy`.

## Procedure

1. **Pick the subcommand** from the user's request. If ambiguous (e.g. "check roy"), default to `roy status`.
2. **Validate the color** if one is required. Must be exactly one of the 7 ROYGBIV tokens, lowercase. Reject anything else with `unknown color: <input> (expected one of red, orange, yellow, green, blue, indigo, violet)`.
3. **For `send`**: quote the text safely. Use a single-arg invocation; do not let the user's text be interpreted by the shell. Prefer passing via a heredoc-free single argument:
   ```
   roy send <color> "<text>"
   ```
   If the text contains both single and double quotes, use Bash with the text passed via an environment variable rather than inline interpolation.
4. **For `start`**: run with `run_in_background: true`. After launch, report the background task id and remind the user to stop it with Ctrl+C in the foreground or by killing the background task.
5. **For `status` / `get`**: run in the foreground and surface the output. If the user asked for JSON, pass `--json` and pretty-print or hand the raw JSON back as a code block.
6. **Errors**: surface `roy`'s stderr verbatim. Do not retry on failure — investigate first.

## What this skill does NOT do

- Does not parse or interpret pane contents — `roy get` output is shown as-is.
- Does not edit `gbiv.md` (use `/gbiv-sync` or `/gbiv-colorize` for that).
- Does not manage tmux sessions directly — go through `roy`.
- Does not invoke `claude` itself inside a worktree — that's the daemon's job.

## Files this skill is allowed to touch

None. This skill only invokes the `roy` binary via Bash.
