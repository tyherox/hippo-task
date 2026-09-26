# 🦛 HippoTask

> **Hippocampus for modern work** — shared, auditable task memory for humans and coding agents.

`hippo-task` keeps a project's tasks in one append-only file in your repo (`.hippotask/ledger.jsonl`), folded into a task list. Several agents — or several chat windows of the same agent — can work from it without stepping on each other: they **claim** work — a claim has no timer: it's theirs until they finish or release it, or a person or orchestrator reclaims it — and every action, including refused claims, stays in an auditable history.

It's the reference implementation of the open agent-native task schema (`docs/schema-design.md`).

**Status: 0.4.0 — internal release** (one machine, one human, many agents). What changed: `CHANGELOG.md`. Agents: read `AGENTS.md`.

*History:* HippoTask started as a TypeScript prototype of a universal interop schema with platform adapters — preserved at git tag `v0-typescript`, with its research (the 10-platform schema study, provider scorecards) in `docs/archive/typescript-v0/`. This Rust core narrows the first release to local, multi-agent task memory; platform adapters come back once sync is earned.

## Install

```bash
make doctor     # checks Rust ≥ 1.89, rustfmt, clippy
make install    # = cargo install --locked --path .   → puts `hippo-task` on your PATH
```

Or run it from this folder: `cargo run -- list`.

## Set up a project

Choose once where a project's tasks live — from anywhere inside it:

```bash
hippo-task init          # asks: in this project (.hippotask/), or another folder
hippo-task init --here   # or choose up front: in this project…
hippo-task init --folder ~/tasks/my-project   # …or in another folder, outside the repository
```

- **Git:** if the tasks would sit inside a git repository, `init` keeps them out of it by default, with a `.gitignore` inside the store folder. It never edits your repository's own `.gitignore`. Pass `--keep-in-git` to let them be committed — then anyone who can read the repository can read every title and note.
- **Another folder:** the project keeps a small pointer, `.hippotask/store.json`, which is always kept out of git (it names a path on this machine).
- **Found from anywhere:** every command looks for the nearest `.hippotask/` in the current folder and the ones above it, never climbing out of a git repository. Without one, commands exit 2 and ask for `hippo-task init` — nothing is ever created by accident.
- **Agents:** add one line to the project's AGENTS.md — *"This project tracks tasks with hippo-task; run `hippo-task guide` before you start."* `hippo-task guide` prints the protocol from the installed binary, so it can't go stale.
- **Session end:** launch each agent window with its own identity (`HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 claude`) and add this Claude Code hook to `.claude/settings.local.json`, so the window's tasks go back when its session ends:

```json
{ "hooks": { "SessionEnd": [ { "hooks": [ { "type": "command", "command": "hippo-task release --all" } ] } ] } }
```

  It runs on `/exit`, `/clear`, logout and resume — not on a crash, which still needs a person to `reclaim`.

## Use

```bash
hippo-task guide                                 # the protocol agents follow
hippo-task add "Write the RFC" --priority high --label docs --body "Scope: the v1 schema"
hippo-task list                                  # every task, by number
hippo-task list --state todo --sort priority     # also: --mine, --blocked, --sort created|updated
hippo-task list --ready --sort priority          # what can be picked up now: open, unblocked, not held
hippo-task list --held                           # who holds what, and how long each has been quiet
hippo-task list --search token_refresh           # before filing: is this already on the list? (title or description)
hippo-task show 3                                # one task + its full history
hippo-task update 3 --priority urgent --label-add api --block 2
hippo-task update 3 --unblock 2 --unassign
hippo-task update 8 --duplicate-of 5             # a duplicate: link it to the original and cancel it
hippo-task start 3                               # claim it + set doing — no timer: yours until done or released
hippo-task note 3 "left off at the token refresh"
hippo-task desc 3 "One paragraph describing the task"
hippo-task release 3                             # hand it back unfinished: back to todo, free for the next worker
hippo-task release --all                         # on exit: give back everything this worker holds
hippo-task reclaim 3 --reason "window closed"    # a person takes back a stuck worker's task (--from <node>: all of it)
hippo-task done 3                                # complete (and release)
```

Every command accepts `--json` (see `AGENTS.md` for the shapes). `release --json` also reports `released`: whether you actually held the task. `release --all` and `reclaim` print an array of the tasks they handed back.

**Task ids:** the number (`3`, shown as `#3`), the full ULID, or 4+ trailing characters of it. In a shell, don't type `#3` unquoted — `#` starts a comment; type `3`.

## Identity: actor + node

- `HIPPO_ACTOR` / `--actor` — *who*: `agent:claude`, `human:ana`. Default: `human:local`.
- `HIPPO_NODE` / `--node` — *which window or session*. Default: `local`.
- **A claim belongs to actor + node.** Give every concurrent worker its own node, and two windows of the same agent can't both claim one task. Agents (`agent:…` actors) *must* set a node — without one, every command exits 2 and says how:

```bash
HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 hippo-task start 3    # → started #3
HIPPO_ACTOR=agent:claude HIPPO_NODE=win-2 hippo-task start 3    # → exit 4: held by agent:claude@win-1 — back off
```

## Exit codes

| exit | kind | meaning |
|---|---|---|
| 0 | — | success |
| 1 | `io` | the ledger couldn't be read, written, or locked |
| 2 | `usage` | invalid input: bad value, ambiguous or too-short id, nothing to do, `--dir` doesn't exist |
| 3 | `not_found` | no task matches the id |
| 4 | `conflict` | held by another worker — or, for `start`, the task is closed; or an agent reclaiming without `--force` |

Errors go to stderr as `error: …` (a JSON line with `--json`); results go to stdout.

## The rules it enforces

- **Claims:** granted if the task is open and nobody else holds it — otherwise exit 4, and the refused attempt is kept in the history as `(rejected)`. A claim has no timer: it holds until its worker finishes or releases it, or someone reclaims it. (Timed leases in ledgers written by 0.1.x still expire as they did.)
- **Whoever knows hands work back:** `release` by the holder returns a started task to `todo`, and `release --all` gives back everything a worker holds — run it from a session's exit hook or a workflow's cleanup. A worker that can't (it crashed, or its window closed) keeps its claim until a person — or the orchestrator that launched it — runs `reclaim` (agents need `--force`); `list --held` shows who's been quiet. Every reclaim is recorded, with its reason.
- **State belongs to the holder:** while someone holds a task, only they can change its state — including completing or cancelling it. Everyone can still edit title, priority, labels, notes. `--force` overrides the state for a human; it doesn't take the claim — `reclaim` does. Closing clears the claim. A closed task can't be started (exit 4) — reopen it with `hippo-task update 3 --state todo`; notes, labels, title, and priority stay editable, and `hippo-task done 3` on a task that's already done is recorded but changes nothing (on a cancelled task it completes it — no refusal).
- **Blocked is derived:** an open task is blocked while any task it's blocked by is still open. A closed task is never blocked.
- **Duplicates are found, then marked:** `list --search` matches the title or description, ignoring case, in any state — check it before filing. `update --duplicate-of` links a duplicate to its original and cancels it, so it never counts as finished work; closing it follows the same holder rule as any other close. A duplicate link never blocks.
- **Merging, not clobbering:** concurrent label/relation changes all survive; for single fields the last write wins. Repeating a change (adding a label twice) is recorded but changes nothing — shown as `(no change)`.

## Data, durability, privacy

- The ledger is `ledger.jsonl` in the project's store — `.hippotask/`, or the folder chosen with `hippo-task init --folder`. Commands find it from the current folder upward; `--dir` / `HIPPO_DIR` names the project folder explicitly instead (it must exist). One JSON event per line, append-only.
- Writes are serialized by a file lock and flushed to disk (fsync) before a command reports success. A write that fails is rolled back, so a failed command never leaves half a change behind. Timestamps strictly increase, so the file's order is the true order of events — even within one millisecond.
- A crash mid-write can leave one unreadable line: every command then warns about it (never silently), and it can't damage later writes.
- **Privacy:** by default nothing about you or your machine is recorded — no username, no hostname. What you type — titles, notes, descriptions, and any actor name you choose — is stored **in cleartext, and forever** (append-only means it can't be edited out). Don't put secrets or personal data in tasks. If you commit `.hippotask/` to git, everyone who can read the repo can read it.

## Develop

```bash
make verify     # fmt --check · clippy -D warnings · tests · test-integrity  (what CI runs)
make test       # also: make lint · make fmt · make doctor · make typecheck
make demo       # narrated demo that drives the real binary
make play       # terminal playground: a REPL over the real binary, with identity switching to try contention
make ui         # local playground: Live mode runs the real binary, Simulate runs in-browser
```

Read the code in this order: `src/model.rs` → `src/fold.rs` (the heart) → `src/store.rs` → `src/ops.rs` → `src/error.rs` → `src/render.rs` → `src/setup.rs` → `src/main.rs`. The tests are the spec: `src/fold.rs` (merge rules + a property test), `tests/store.rs`, `tests/ops.rs`, `tests/cli.rs`, `tests/setup.rs`, `tests/docs.rs`.

## Scope

- **In 0.4.0:** single-file ledger in a store chosen with `init` and found from anywhere in the project; fold to state; twelve commands; actor + node identity; claims without timers, handed back by release or reclaim; search, and duplicates linked and cancelled; derived blocked; JSON + exit-code contract; locking, fsync, crash tolerance.
- **Deferred until real use earns them (staging rule):** full Hybrid Logical Clock, per-task hash-chaining, snapshots/compaction, storage adapters, multi-machine sync, a GUI. The schema leaves room for each without a breaking change.

## Troubleshooting

- **`warning: …ledger.jsonl:N: skipped an unreadable line`** — line N is damaged (usually a crash mid-write). Everything else still works. To silence it, delete that one line by hand.
- **`error: couldn't lock …`** — another `hippo-task` process held the ledger for over 10 s (a hung or suspended process?). Find and stop it, then retry.
- **`error: no task store here or in any folder above it`** — this project hasn't chosen where its tasks live. A person runs `hippo-task init` (from anywhere inside the project).
- **`error: no such directory`** — `--dir` / `HIPPO_DIR` must point at an existing folder; `hippo-task` won't create one for you (a typo would silently start a new, empty list).

## License

MIT — see [LICENSE](LICENSE).
