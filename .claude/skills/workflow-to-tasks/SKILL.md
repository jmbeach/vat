---
name: workflow-to-tasks
description: Convert a workflow TOML file into tasks using the TaskCreate tool. Use whenever the user provides (or points at) a `.toml` file that defines `[[steps]]`, mentions a "workflow file", "formula", or asks to load/apply/run/instantiate a workflow. Also use when the user says things like "add these steps as tasks", "create tasks from this workflow", "turn this into a task list", or drops a workflow-shaped TOML into the chat — even if they don't explicitly name the TaskCreate tool.
---

# Workflow to tasks

This skill turns a workflow TOML file into a set of tasks in the current session's task list. The workflow encodes steps, their dependencies, and variables that need to be filled in; your job is to resolve those variables, create one task per step, and wire up the `needs` relationships.

## Input format

A workflow TOML looks like this:

```toml
formula = "feature-workflow"
description = "Standard feature development workflow"
version = 1
type = "workflow"

[vars.feature_name]
description = "Name of the feature"
required = true

[[steps]]
id = "design"
title = "Design {{feature_name}}"
type = "human"
description = "Create design document"

[[steps]]
id = "implement"
title = "Implement {{feature_name}}"
needs = ["design"]
```

Fields you care about:

- **Top-level `formula`** — a name for the workflow. Useful as metadata on each task.
- **`[vars.NAME]`** — a variable declaration. `required = true` means a value must be supplied before tasks can be created. References to it inside step titles/descriptions use the `{{NAME}}` syntax.
- **`[[steps]]`** — one table per step, in no particular order. Fields:
  - `id` (required): short identifier used by other steps' `needs`.
  - `title` (required): becomes the task subject. May contain `{{var}}` placeholders.
  - `description` (optional): fuller description. Also may contain placeholders.
  - `needs` (optional): list of step `id`s that must complete before this one starts.
  - `type` (optional): `"human"` means a person does this step, not Claude. Other values are free-form metadata.

## Workflow for you

### 1. Parse the TOML

Use Python's `tomllib` (stdlib, 3.11+) or `tomli` rather than parsing by hand — hand-parsing is error-prone on edge cases (quoted strings with braces, multiline values). A one-liner works:

```bash
python3 -c "import tomllib, json, sys; print(json.dumps(tomllib.load(open(sys.argv[1],'rb'))))" <path>
```

### 2. Resolve variables

Look at every `[vars.NAME]` entry. For each:

- If the user already supplied a value in their message (or an earlier turn), use it.
- Otherwise, if `required = true`, ask the user for it. **Ask for all missing required vars in a single message**, using each var's `description` as the prompt — don't ping-pong one at a time.
- If `required` is false or absent and no value is given, leave the placeholder empty (substitute with `""`) unless context makes a better default obvious.

Then substitute `{{NAME}}` occurrences in every step's `title` and `description`. If a placeholder references an undeclared variable, flag it to the user rather than silently leaving it as literal text.

### 3. Create tasks

For each step, call `TaskCreate` with:

- `subject`: the resolved title. Keep it short and imperative — it's what the user sees in the task list.
- `description`: the resolved description if present; otherwise reuse the title. If `type == "human"`, prepend `[human] ` so it's visually obvious this step isn't for Claude to execute.
- `activeForm` (optional but nice): a natural present-continuous phrasing — "Implement auth" → "Implementing auth". Skip it if the transform feels awkward.
- `metadata`: `{"step_id": <id>, "formula": <formula-name>, "type": <type or null>}`. This lets you look tasks back up by their workflow-step id when wiring dependencies and gives future tools a way to recognize workflow-originated tasks.

**Remember the mapping** from each step's `id` to the task id that `TaskCreate` returns — you'll need it in the next step.

Batch these `TaskCreate` calls in a single response where possible; they're independent.

### 4. Wire up dependencies

After all tasks exist, for every step with a non-empty `needs`, call `TaskUpdate` on that step's task with `blockedBy` set to the task ids corresponding to the named step `id`s. If a `needs` entry references a step id that doesn't exist in the file, flag that to the user — it's almost certainly a typo in the workflow.

### 5. Confirm

Give the user a short summary: the formula name, how many tasks were created, and a call-out of any steps marked `type = "human"` so they know those expect manual completion. No need to list every task — the task list UI shows that.

## Edge cases

- **No `[[steps]]` entries**: tell the user the file has no steps and stop — don't invent any.
- **Duplicate step `id`s**: flag it and stop. Dependencies become ambiguous.
- **Circular `needs`**: detect before creating tasks (a simple DFS suffices) and flag it. Creating the tasks first and then failing to wire them leaves a messy half-state.
- **`{{var}}` placeholders in fields other than `title`/`description`**: substitute in those too, in case a future workflow uses them.
- **Non-workflow TOML**: if the file is missing `[[steps]]` entirely, it's probably not a workflow file. Ask the user to confirm before proceeding.
