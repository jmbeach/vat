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

Install from crates.io with Cargo. The crate is published as `vat-cli`, and it installs a binary named `vat`:

```sh
cargo install vat-cli
```

The bare `vat` name was already taken on crates.io, so the crate is `vat-cli` — but the command you run is still `vat`.

To track unreleased `main` instead, install straight from the repo:

```sh
cargo install --git https://github.com/jmbeach/vat
```

This builds and installs the latest `main`. To pin a specific release, add `--tag <version>` (e.g. `--tag v1.0.0`).

### Prebuilt binaries

Every tagged release also carries prebuilt binaries, attached as `.tar.gz` assets to the [GitHub Release](https://github.com/jmbeach/vat/releases) for that tag. The release workflow builds three targets:

- `aarch64-apple-darwin` — Apple Silicon macOS
- `x86_64-apple-darwin` — Intel macOS
- `x86_64-unknown-linux-musl` — x86-64 Linux (statically linked)

The Linux binary is built against musl and statically linked, so it runs on any x86-64 Linux — including musl-based distros (e.g. Alpine) and older glibc systems — with no runtime libc dependency.

Download the archive for your platform, extract the `vat` binary, and put it somewhere on your `PATH`:

```sh
tar -xzf vat-v1.0.0-aarch64-apple-darwin.tar.gz
install -m 755 vat /usr/local/bin/vat
```

No-install option: hand an agent the [`vat` skill](./.claude/skills/vat/SKILL.md) and it operates a backlog using only file edits and `git` — no `cargo`, no binary.

At it's core, `vat` is just a markdown file that has single line entries for each task in the backlog. If the information needed to capture the task fully extends past one line, `vat` stores the rest of the information of that item in a file under `backlog/items/<id>.md` You can add tags to a backlog item using square brackets. Ex: `[in-progress]` or `[by:jared]`.

The `vat` skill and cli support claiming tasks, creating them / assigning them ID's, completing them, etc.

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
