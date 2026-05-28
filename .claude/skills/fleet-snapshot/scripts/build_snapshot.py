#!/usr/bin/env python3
"""Build a fleet-snapshot.md aggregating state across roy panes.

Pure aggregator. Runs `roy status --lines 60`, locates each color's worktree,
pulls in any `.claude-cache/<task-id>-questions.md` and `.claude-cache/<task-id>-pr.url`
artifacts that already exist on disk, appends a context bundle (HLD / LLDs /
specs / in-flight task notes / referenced src files), and writes the result to
`.claude-cache/fleet-snapshot.md`.

Does NOT decide whether any pane needs a nudge — that judgment belongs to the
caller (the SKILL.md walks Claude through reading `roy status` and deciding).

Output to stdout: JSON of the form
    {"snapshot": "<path>", "panes": [{"color": ..., "task_id": ..., "tail": "..."}]}
The `tail` field is the last ~40 non-blank, non-chrome lines of the pane,
intended for the caller to read and classify.

Exit codes:
  0 success
  1 roy daemon not reachable / unexpected error
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

COLORS = ["red", "orange", "yellow", "green", "blue", "indigo", "violet"]
WORKTREE_BASE = Path.home() / "code" / "sandbox" / "vat"
TASK_ID_RE = re.compile(r"vat-[0-9a-z]{3}")
QUESTIONS_FILENAME_RE = re.compile(r"^(vat-[0-9a-z]{3})-questions\.md$")
SRC_FILE_RE = re.compile(r"\bsrc/[a-zA-Z0-9_/]+\.rs\b")
BARE_RS_RE = re.compile(r"\b([a-z][a-z0-9_]*)\.rs\b")
OUT_PATH = Path(".claude-cache/fleet-snapshot.md")
REPO_ROOT = Path.cwd()

CHROME_PATTERNS = (
    "─────",  # divider rule
    "[  Opus",  # status bar model badge
    "⏵⏵ accept edits",
    "Remote Control active",
)

HANDOFF_PROMPT = """\
# Handoff prompt — read this first

You are being handed a snapshot of multiple parallel coding sessions working on
a Rust CLI called **VAT** (Versioned Addressable Tasks). Each "color" (red,
orange, yellow, green, blue) is an independent Claude Code session, each on its
own git worktree and branch, each working a separate task from the project
backlog. The user is orchestrating them and wants to discuss the open decisions
*verbally* with you.

## What's in this file

1. **Fleet snapshot** (next section) — one block per color, with:
   - the task ID and most-recent pane tail (sanitized terminal output) so you
     can see what that session was doing,
   - the session's `questions.md` file inline, which captures the design
     decisions it's currently asking the user to make, with options and
     pros/cons for each.
2. **Context appendix** — the project's high-level design, every low-level
   design, every EARS spec file, the in-flight task notes
   (`backlog/items/vat-*.md`), and every `src/*.rs` file referenced by the
   questions docs. This is the full context you need to discuss the decisions
   intelligently. You do not need to ask for any additional files.

## How to engage

Open by briefly acknowledging which colors are active and what each one is
asking about (one line each). Then ask the user which task they want to dig
into first. Do not try to answer every open question up front — wait for the
user to pick a thread.

When discussing a specific decision, refer to it by task ID + question number
(e.g. "vat-r2n Q11"). Quote option labels verbatim when comparing them. If you
think one of the listed options is clearly best, say so and explain why — but
remember the user is the decider; your job is to surface tradeoffs they may
have missed, not to pick for them.

If you spot a gap (a decision the session didn't capture, a contradiction
between specs, a missing edge case), flag it directly.

---

"""


def roy_status() -> list[dict]:
    if not shutil.which("roy"):
        print("error: `roy` CLI not on PATH", file=sys.stderr)
        sys.exit(1)
    try:
        out = subprocess.run(
            ["roy", "status", "--lines", "60"],
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError as e:
        print(f"error: roy status failed (exit {e.returncode}):\n{e.stderr}", file=sys.stderr)
        sys.exit(1)
    return json.loads(out.stdout)


def find_task_id(color: str, output: str | None) -> str | None:
    """Prefer the questions doc filename; fall back to scanning pane output."""
    cache = WORKTREE_BASE / color / "vat" / ".claude-cache"
    if cache.is_dir():
        for entry in cache.iterdir():
            m = QUESTIONS_FILENAME_RE.match(entry.name)
            if m:
                return m.group(1)
    if output:
        matches = TASK_ID_RE.findall(output)
        if matches:
            return matches[0]
    return None


def sanitize_tail(output: str | None, max_lines: int = 40) -> str:
    """Strip status-bar chrome and blank lines; return the trailing meaningful lines."""
    if not output:
        return ""
    kept: list[str] = []
    for line in output.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if any(p in stripped for p in CHROME_PATTERNS):
            continue
        kept.append(stripped)
    return "\n".join(kept[-max_lines:])


def read_text(path: Path | None) -> str | None:
    if path is None:
        return None
    try:
        return path.read_text()
    except (FileNotFoundError, IsADirectoryError):
        return None


def file_section(path: Path, fence: str | None = None) -> str:
    try:
        rel = path.relative_to(REPO_ROOT)
    except ValueError:
        rel = path
    text = read_text(path)
    if text is None:
        return f"### `{rel}`\n\n_(not found)_\n"
    body = text.rstrip()
    if fence:
        body = f"```{fence}\n{body}\n```"
    return f"### `{rel}`\n\n{body}\n"


def build_pane_section(color: str, pane: dict) -> tuple[str, dict]:
    """Build the snapshot section for one pane + a small dict describing it."""
    info = {"color": color, "pane_status": pane.get("pane_status", "unknown")}
    if info["pane_status"] != "ok":
        return f"## {color}\n\n**pane_status:** `{info['pane_status']}`\n", info

    output = pane.get("output") or ""
    task_id = find_task_id(color, output)
    info["task_id"] = task_id
    info["tail"] = sanitize_tail(output)

    cache = WORKTREE_BASE / color / "vat" / ".claude-cache"
    q_path = cache / f"{task_id}-questions.md" if task_id else None
    pr_path = cache / f"{task_id}-pr.url" if task_id else None
    q_text = read_text(q_path)
    pr_url = (read_text(pr_path) or "").strip() or None
    info["has_questions_doc"] = bool(q_text)
    info["pr_url"] = pr_url

    lines = [f"## {color} — {task_id or '(task id unknown)'}", ""]
    if pr_url:
        lines.append(f"- **PR:** {pr_url}")
    if q_path and q_text:
        lines.append(f"- **Questions doc:** `{q_path}`")
    lines.append("")
    lines.append("### Recent pane tail")
    lines.append("")
    lines.append("```")
    lines.append(info["tail"])
    lines.append("```")
    lines.append("")
    if q_text:
        lines.append(f"### Questions doc — `{q_path.name}`")
        lines.append("")
        lines.append(q_text.rstrip())
        lines.append("")
    return "\n".join(lines), info


def collect_source_files(texts: list[str]) -> list[Path]:
    found: set[str] = set()
    for t in texts:
        if not t:
            continue
        found.update(SRC_FILE_RE.findall(t))
        for stem in BARE_RS_RE.findall(t):
            found.add(f"src/{stem}.rs")
    paths: list[Path] = []
    for rel in sorted(found):
        p = REPO_ROOT / rel
        if p.is_file():
            paths.append(p)
    return paths


def build_appendix(in_flight_task_ids: list[str], extra_texts: list[str]) -> str:
    out: list[str] = ["", "---", "", "# Context appendix", "",
                      "_Bundled for handoff to a fresh chat. Trim sections you don't need._", ""]

    out.append("## High-level design\n")
    hld = REPO_ROOT / "docs" / "high-level-design.md"
    out.append(file_section(hld) if hld.is_file() else "_(docs/high-level-design.md not found)_\n")

    out.append("## Low-level designs\n")
    lld_dir = REPO_ROOT / "docs" / "llds"
    if lld_dir.is_dir():
        for p in sorted(lld_dir.glob("*.md")):
            out.append(file_section(p))
    else:
        out.append("_(docs/llds/ not found)_\n")

    out.append("## EARS specs\n")
    specs_dir = REPO_ROOT / "docs" / "specs"
    if specs_dir.is_dir():
        for p in sorted(specs_dir.glob("*.md")):
            out.append(file_section(p))
    else:
        out.append("_(docs/specs/ not found)_\n")

    out.append("## In-flight task notes\n")
    items_dir = REPO_ROOT / "backlog" / "items"
    for tid in in_flight_task_ids:
        out.append(file_section(items_dir / f"{tid}.md"))

    bundled_texts = list(extra_texts)
    for tid in in_flight_task_ids:
        t = read_text(items_dir / f"{tid}.md")
        if t:
            bundled_texts.append(t)
    src_files = collect_source_files(bundled_texts)
    out.append("## Source files referenced\n")
    if src_files:
        for p in src_files:
            out.append(file_section(p, fence="rust"))
    else:
        out.append("_(no src/*.rs paths referenced)_\n")

    return "\n".join(out)


def main() -> None:
    # Optional positional arg: a single color name to filter to.
    only_color: str | None = None
    if len(sys.argv) > 1:
        arg = sys.argv[1].lower()
        if arg not in COLORS:
            print(f"error: unknown color `{arg}`; must be one of {COLORS}", file=sys.stderr)
            sys.exit(1)
        only_color = arg

    out_path = OUT_PATH
    out_path.parent.mkdir(parents=True, exist_ok=True)
    sessions = roy_status()
    by_color = {s["color"]: s for s in sessions}
    sections: list[str] = []
    panes_info: list[dict] = []
    in_flight_task_ids: list[str] = []
    questions_texts: list[str] = []
    for color in COLORS:
        if only_color is not None and color != only_color:
            continue
        pane = by_color.get(color)
        if pane is None:
            continue
        section, info = build_pane_section(color, pane)
        sections.append(section)
        panes_info.append(info)
        tid = info.get("task_id")
        if tid and tid not in in_flight_task_ids:
            in_flight_task_ids.append(tid)
        cache = WORKTREE_BASE / color / "vat" / ".claude-cache"
        if tid:
            qt = read_text(cache / f"{tid}-questions.md")
            if qt:
                questions_texts.append(qt)

    title = f"# Fleet snapshot — {only_color}" if only_color else "# Fleet snapshot"
    main_body = "\n".join([title, ""] + sections).rstrip() + "\n"
    appendix = build_appendix(in_flight_task_ids, questions_texts)
    out_path.write_text(HANDOFF_PROMPT + main_body + appendix.rstrip() + "\n")

    print(json.dumps({"snapshot": str(out_path), "panes": panes_info}, indent=2))


if __name__ == "__main__":
    main()
