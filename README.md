# VAT

VAT stands for "Versioned, Addressable, Tasks".

- "Versioned" because you can track your tasks in source control.
- "Addressable" because each task gets an ID that you can use to reference (address) the task - ex: "vat-f1w"
- "Tasks" goes without saying

`vat` was in all honesty designed because agents need an easy way to claim work in a backlog without stepping on eachother's toes.

But `vat` is great for both agents and humans.

It's low friction, compact, and simple.

`vat` has two first-class forms: the Rust binary (the primary distribution) and the [skill](./.claude/skills/vat/SKILL.md) — a prose implementation an agent follows directly, with zero install. I built `vat` *with* `vat`, using the skill to operate the backlog before the binary was finished. Take a look at the [backlog](https://github.com/jmbeach/vat-backlog/blob/main/backlog.md) to see a good example of what a `vat` backlog looks like and the current state of the project.

## Install

`vat` is not yet published to crates.io. For v1, install the binary straight from the repo with Cargo:

```sh
cargo install --git https://github.com/jmbeach/vat
```

This builds and installs the latest `main`. To pin a specific release, add `--tag <version>` (e.g. `--tag v1.0.0`).

### Prebuilt binaries

Every tagged release also carries prebuilt binaries, attached as `.tar.gz` assets to the [GitHub Release](https://github.com/jmbeach/vat/releases) for that tag. The release workflow builds three targets:

- `aarch64-apple-darwin` — Apple Silicon macOS
- `x86_64-apple-darwin` — Intel macOS
- `x86_64-unknown-linux-gnu` — x86-64 Linux

Download the archive for your platform, extract the `vat` binary, and put it somewhere on your `PATH`:

```sh
tar -xzf vat-v1.0.0-aarch64-apple-darwin.tar.gz
install -m 755 vat /usr/local/bin/vat
```

No-install option: hand an agent the [`vat` skill](./.claude/skills/vat/SKILL.md) and it operates a backlog using only file edits and `git` — no `cargo`, no binary.

At it's core, `vat` is just a markdown file that has single line entries for each task in the backlog. If the information needed to capture the task fully extends past one line, `vat` stores the rest of the information of that item in a file under `backlog/items/<id>.md` You can add tags to a backlog item using square brackets. Ex: `[in-progress]` or `[by:jared]`.

The `vat` skill and (future) cli support claiming tasks, creating them / assigning them ID's, completing them, etc.
