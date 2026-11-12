# Contributing

**To the book** (a typo, an unclear explanation, a correction): see
[Contributing To This Book](README.md#contributing-to-this-book) in the README.
Open an issue first, then a pull request.

**To the code companion** in [`code/`](code/): there are two ways in, and both
go through the same gate.

- **Lend an agent.** Run its tasks through [AgentFund](https://agentfund.dev).
  The worker does everything described below for you, inside a sandbox. The
  [README](README.md#code-companion) has the commands.
- **Write the code yourself.** Pick a task, implement it, open a pull request.
  You follow the same rules as the agents, without the sandbox.

Either way, **CI is the only reviewer that can say yes.** The maintainer can
still say no.

## The mechanism

| Piece | Where | Who may change it |
| --- | --- | --- |
| task manifest | `tasks/<id>.yaml` | the maintainer (re-signing the lock) |
| acceptance test | `code/acceptance/t<id with _>.rs`, `code/acceptance/common/` | the maintainer |
| the code | the files in the manifest's `scope.write` | whoever takes the task |
| done marker | `code/ci/done/<id>` (empty file) | whoever takes the task |
| the gate | `code/ci/*.sh`, `.github/workflows/companion.yml` | the maintainer |

A task can be claimed once everything in its `depends_on` is merged.
`sh code/ci/scope-audit.sh layers` prints the dependency layers. A task whose
marker is in `code/ci/done/` is finished.

```
layer 0   0001-tun-device (ch. 1)       0002-parsing-the-bits (ch. 2)
layer 1   0003-connection-quad (ch. 3)  0004-final-handshake (ch. 4)     both need 0002
layer 2   0005-listening-socket (ch. 5)                                  needs 0003 and 0004
```

A task is **done** when its branch changes only the files in its
`scope.write`, and these pass:

```sh
: > code/ci/done/<id>          # create the empty marker
sh code/ci/check.sh <id>       # must end with "check.sh: OK"
```

`check.sh` builds the crate, runs the unit tests, compiles every acceptance
target (so no public signature has drifted), and runs clippy if it is
installed. Then it runs the acceptance test of every task with a marker,
including yours. If you have no Rust toolchain:

```sh
docker run --rm -v "$PWD":/src -w /src rust:1-slim sh code/ci/check.sh <id>
```

## Rules for a task pull request

1. **Stay in scope.** Change only the paths in the manifest's `scope.write`.
   `sh code/ci/scope-audit.sh diff <id> origin/main` tells you whether you
   did. An AgentFund worker throws away any run that strays. A human's PR
   that strays will be asked to split.
2. **Do not edit the contract or the judge.** That means `code/Cargo.toml`,
   `code/Cargo.lock`, `code/src/lib.rs`, `code/src/main.rs`, any `mod.rs`,
   `code/acceptance/`, `code/ci/*.sh`, `tasks/`, `.github/`, `agentfund.yaml`
   and the book's chapters. If one of them is wrong, the task set has a bug:
   open an issue instead of working around it.
3. **Keep the public signatures.** Someone you will never talk to is writing
   another chapter against them right now.
4. **Standard library only.** No dependencies. Task sandboxes have no route
   to crates.io.
5. **No panics on anything a peer can send.** `unsafe` is allowed in exactly
   one place, the `ioctl` call in `code/src/tun.rs`.
6. **Add unit tests next to your code** (`#[cfg(test)] mod tests`) if they
   help you. They run in the gate, but they do not replace the acceptance test.
7. **One task per pull request**, with the task id in the title.
8. **Never remove or edit a marker.** CI rejects pull requests that do.

## Things that are not tasks

For a bug in merged code, a wrong acceptance test, an ambiguous prompt, or
code for a new chapter: open an issue. Fixes to merged code are ordinary
pull requests, and every marked acceptance test must stay green. A new
chapter arrives as new manifests. The maintainer writes the stubs, the
acceptance test and the manifest, re-signs the lock, and the work becomes
claimable.

## For the maintainer: changing the task set

```sh
$EDITOR tasks/0006-something.yaml code/acceptance/t0006_something.rs code/src/...
# add the [[test]] target to code/Cargo.toml
sh code/ci/check.sh                       # the skeleton still builds, old tasks still pass
: > code/ci/done/0006-something && sh code/ci/check.sh 0006-something; rm code/ci/done/0006-something
                                          # ^ must FAIL: a task that passes before it is done is not a task
sh code/ci/scope-audit.sh collisions      # no two open tasks share a file
agentfund validate --strict tasks/
agentfund sign tasks/ --key ~/.config/agentfund/keys/rust-tcp.key
git add tasks/ code/ && git commit -m "tasks: ..."
```

Any change under `tasks/` changes `tasks/MANIFEST.lock`. Every donor then has
to approve again, so batch your edits.
