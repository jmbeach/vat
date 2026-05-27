---
name: backlog-sync
description: Sync local backlog/ changes to GitHub via a short-lived chore PR. Use when the user asks to "sync the backlog", "push backlog changes", "commit the backlog updates", "open a backlog PR", or similar. Creates a branch, commits backlog/ changes (and optionally any other dirty files), opens a PR, squash-merges it, and switches back to main. Skip if backlog/ is clean.
argument-hint: (no args)
allowed-tools: Bash, Read
---

# backlog-sync

Pushes pending `backlog/` changes to GitHub via a short-lived chore PR. Always lands back on `main` when done.

## Flow

### 1. Check git state

```bash
git status --porcelain
git branch --show-current
```

Categorize the working tree:
- **Backlog changes** — anything under `backlog/` (modified, staged, or untracked).
- **Other changes** — everything else.

If backlog has nothing dirty, tell the user `backlog/ is clean — nothing to sync` and stop.

### 2. Handle other dirty files

If there are other changes, list them to the user and ask via AskUserQuestion which they want:

1. **Fold into the chore commit** — `git add` them alongside backlog and commit together.
2. **Separate commit on the same branch** — backlog gets its own commit, others get one or more follow-up commits (ask for a message). All ride in the same PR.
3. **Leave alone** — only backlog goes into the PR; other files stay dirty in the working tree.

Do not assume; always ask.

### 3. Confirm starting branch

Determine the project's main branch:

```bash
git symbolic-ref --short refs/remotes/origin/HEAD | sed 's@^origin/@@'
```

(Fall back to `main` if that returns nothing.)

If the current branch is NOT the main branch, ask the user before proceeding — they may have intended to push from a feature branch.

If you are on main, pull first so the new branch starts from latest:

```bash
git pull --ff-only
```

If `--ff-only` fails (diverged main), surface the error and stop. Do not reset or force.

### 4. Create the branch

```bash
git checkout -b "chore/backlog-sync-$(date +%Y%m%d-%H%M%S)"
```

(Timestamp avoids collisions if the skill is run twice in quick succession.)

### 5. Stage and commit

```bash
git add backlog/
# also stage other files if user chose option 1 in step 2
git commit -m "chore: sync backlog"
```

If the user chose option 2 (separate commits), do a second `git add` + `git commit -m "<their message>"` after the chore commit.

Never pass `--no-verify`. Never use `--amend` on a previous commit.

### 6. Push

```bash
git push -u origin HEAD
```

### 7. Open the PR

```bash
gh pr create --title "chore: sync backlog" --body "Sync of local backlog/ changes."
```

Capture the URL from the output.

### 8. Squash-merge

```bash
gh pr merge --squash --delete-branch
```

This merges into main, deletes the remote branch, and waits for the merge. If it fails (branch protection, required CI, required reviews), surface the error verbatim and stop — do NOT bypass with `--admin` or any force flag.

### 9. Return to main

```bash
git checkout <main-branch>
git pull --ff-only
git branch -d <chore-branch>   # local cleanup; -d is safe (refuses if unmerged)
```

If `git branch -d` complains, leave the local branch alone and tell the user. Do not use `-D`.

### 10. Report

Tell the user:
- The PR URL.
- One-line summary of what synced (file count, or specific filenames if few).
- Confirmation they're back on main at the new HEAD.

## Safety guards

- Never `--no-verify` on commit or push.
- Never force-push (`--force`, `--force-with-lease`).
- Never `--amend` an existing commit; always create a new one.
- Never use `gh pr merge --admin` to bypass protection rules.
- Never `git reset --hard` or `git checkout --` to resolve issues — surface and ask.
- Do not run if a merge/rebase is in progress (`git status` will show it). Tell the user to finish or abort first.

## When NOT to invoke

- The user wants to commit something *other* than backlog (use `/cbcp` or plain git).
- The repo has uncommitted in-progress conflict resolution.
- The user wants a draft PR (this skill always merges immediately).

## Files this skill touches

- The git index, working tree (only via `git add` of files already present), the remote, and the merged PR.
- Does not modify any project files directly.
