# VAT

VAT stands for "Versioned, Addressable, Tasks".

- "Versioned" because you can track your tasks in source control.
- "Addressable" because each task gets an ID that you can use to reference (address) the task - ex: "vat-f1w"
- "Tasks" goes without saying

`vat` was in all honesty designed because agents need an easy way to claim work in a backlog without stepping on eachother's toes.

But `vat` is great for both agents and humans.

It's low friction, compact, and simple.

In fact, right now the binary for `vat` is unfinished, but I've been using it tons in the [skill](./.claude/skills/vat/SKILL.md) form. Not only that, but I've been using `vat` to build `vat` before its completion using the skill. Take a look at the [backlog](https://github.com/jmbeach/vat-backlog/blob/main/backlog.md) to see a good example of what a `vat` backlog looks like and the current state of the project.

At it's core, `vat` is just a markdown file that has single line entries for each task in the backlog. If the information needed to capture the task fully extends past one line, `vat` stores the rest of the information of that item in a file under `backlog/items/<id>.md` You can add tags to a backlog item using square brackets. Ex: `[in-progress]` or `[by:jared]`.

The `vat` skill and (future) cli support claiming tasks, creating them / assigning them ID's, completing them, etc.
