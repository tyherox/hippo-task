# AGENTS.md — HippoTask (`hippo-task`)

Two audiences: **A** is for agents *using* `hippo-task` to coordinate work (copy it into your project's own AGENTS.md). **B** is for agents *changing* this crate.

## A. Coordinating work with `hippo-task`

Set once per session — and give every window or session its own node:

```bash
export HIPPO_ACTOR=agent:claude       # who you are
export HIPPO_NODE=claude-win-1        # unique per window/session; required for agent:… actors (exit 2 without it)
```

The loop:

1. **Pick.** `hippo-task list --json --ready --sort priority` — every task listed is open, unblocked, and free, including work another agent handed back. Take the first that fits.
2. **Claim.** `hippo-task start 3 --json` — exit 0: it's yours (claimed + state doing) until you finish or release it. There's no timer and nothing to renew. Exit 4: someone else has it — pick another; don't retry the same task.
3. **Work, and leave breadcrumbs.** `hippo-task note 3 "what I did / where I stopped"` — the next agent reads these in `hippo-task show 3 --json`. Each note is also a sign of life: `list --held` shows how long a holder has been quiet.
4. **Finish** with `hippo-task done 3`. Stopping without finishing? Add a note saying why, then `hippo-task release 3` — it goes back to `todo` for the next agent.
5. **On exit,** `hippo-task release --all --json` gives back anything you still hold. Run it from your session's exit hook or your workflow's cleanup: a claim you don't give back stays yours until a person reclaims it.

Rules:

- Always pass `--json`, parse stdout, and branch on the exit code — never scrape the text output.
- Refer to tasks by number — `3`, not `#3` (in a shell, `#` starts a comment).
- Don't change the state of (or close) a task someone else holds — the CLI refuses with exit 4. `--force` and `hippo-task reclaim` are for humans, and for the orchestrator that launched a worker (to take back a failed worker's tasks: `reclaim --from <node> --force`) — not for you.
- Never edit `.hippotask/ledger.jsonl` by hand: it's append-only and the CLI is its only writer.
- Before you `add` a task, search for the most distinctive term in it — a function, a file, an error message: `hippo-task list --json --search token_refresh`. If a task already covers it, add a note there instead. If you find you're working on a duplicate, mark it: `hippo-task update 8 --duplicate-of 5 --json` links it to the original and cancels it.
- Never put secrets or personal data in titles, notes, or descriptions — they are stored in cleartext, forever.

### Exit codes

| exit | kind | meaning | what to do |
|---|---|---|---|
| 0 | — | success | carry on |
| 1 | `io` | the ledger couldn't be read, written, or locked | if the message says "rolled back", retry once; otherwise check `hippo-task show` first. If it persists, tell the human |
| 2 | `usage` | invalid input (bad value, ambiguous id, nothing to do) | fix the command; don't retry it unchanged |
| 3 | `not_found` | no task matches the id | re-list; the number may be wrong |
| 4 | `conflict` | held by another worker — or, for `start`, the task is closed; or an agent reclaiming without `--force` | pick another task — don't force |

Closed tasks (`done` or `cancelled`): only `start` refuses them. `hippo-task update 3 --state todo` reopens one; notes, labels, title, and priority edits are accepted on a closed task; `hippo-task done 3` on a task that's already done is recorded but changes nothing (exit 0, `applied: false` in its history), and on a cancelled task it completes it (no refusal).

### JSON shapes

With `--json`, stdout is exactly one JSON document. Every single-task command (`add`, `update`, `start`, `release`, `note`, `desc`, `done`, `show`) prints a **task object**; `list`, `release --all`, and `reclaim` print an array of them (for the last two, the tasks they handed back — possibly none).

Task object:

- `id` — the ULID; the permanent identifier
- `num` — the friendly number (`#3`); stable on one machine
- `title`; `body` — the description, or null
- `state` — `todo` | `doing` | `done` | `cancelled`
- `priority` — `none` | `low` | `med` | `high` | `urgent`
- `assignee` — who *should* own it (durable intent, not a claim), or null
- `labels` — sorted array of strings
- `relations` — array of `{"rel": "blocked-by" | "duplicate-of", "task": "<id>"}`; only `blocked-by` affects `blocked`
- `blocked` — derived: true while any blocked-by task is still open
- `lease` — who holds the task: null, or `{"holder", "node", "expires_ms", "active", "since_ms", "last_seen_ms"}`. `holder` + `node` are the worker; `expires_ms` is null for a claim (it holds until released, the task closes, or someone reclaims it) or unix millis for a timed lease from a 0.1.x ledger; `active` is whether it was in force when the command ran; `since_ms` is when this worker's hold began; `last_seen_ms` is the holder's latest event on the task — its last sign of life
- `created_ms`, `updated_ms` — unix millis; `seq` — how many content changes the task has had

`show --json` adds `events`: the task's full history in order. Each event is exactly the ledger line — `eid`, `task`, `ts`, `actor`, `node`, `type`, `data` — plus `applied`: false means it was recorded but had no effect (a rejected claim, a repeated change).

`release --json` adds `released`: true if you held the task and gave it back; false if you didn't hold it, so nothing changed (still exit 0 — either way you don't hold it afterwards).

On stderr, `--json` mode writes JSON lines: zero or more `{"warning": "…"}`, then — on failure — one error object with the fields `error` (the kind), `message`, and `exit_code`, e.g. `{"error":"conflict","message":"#3 is held by agent:codex@cx (quiet 7m) — back off and pick another task","exit_code":4}`.

Stability: within 0.3.x, fields are only ever added — never renamed or removed. Anything breaking bumps the version and is called out in CHANGELOG.md.

## B. Changing this crate

- **Decide before you build.** A new feature or a domain-model change starts with a short decision record in `docs/decisions/` (like ADR-001), before any code.
- **Done means `make verify` passes** — fmt, clippy with warnings as errors, every test, and the test-integrity gate. CI runs the same thing on Linux and macOS, plus `cargo test --locked` on the minimum supported Rust (1.89). Check the machine first with `make doctor`.
- **Test first.** Write the failing test, then the code. Tests are the guardrail: strengthen them freely, but never weaken or delete one to get a change through — the integrity gate (`make integrity`, run by `make verify` and CI) fails on removed assertions and newly ignored tests. It only runs when it can resolve a base commit to diff against (no git history, or an unknown base: it says so and passes). A genuine exception needs `test-weaken-ok: <reason>` in the diff — one such line waives the check for the whole diff, so it needs a human review.
- **No panics in product code.** `unwrap`, `expect`, `panic!`, and `println!` are denied clippy lints (Cargo.toml `[lints.clippy]`): `cargo build` still compiles, but `make verify`'s `clippy -D warnings` fails. Return an `Error` from `src/error.rs`, with context. Never swallow an error: at minimum report it (see how the ledger reader skips *and reports* bad lines).
- **Physics vs etiquette.** Merge rules live in `src/fold.rs` and must hold for any ledger in any order — a hand-rolled property test there checks it (300 random ledgers, each folded in 3 shuffled orders, plus invariants; no proptest crate). CLI policy ("only the holder may close a task") lives in `src/ops.rs`.
- **The on-disk event shape is frozen** (test in `src/model.rs`). New event kinds may be added; existing ones never change shape.
- **Privacy by default.** Collect nothing about the user or machine unless they opt in (guarded by `tests/cli.rs`). Flag any change that would record personal data before making it.
- **Docs are tested** (`tests/docs.rs`): a new command must appear in README.md, a new JSON field here, a version bump in CHANGELOG.md.
- **Don't over-engineer.** Full HLC, hash-chaining, compaction, sync, storage adapters, and a GUI stay deferred until real use earns them.
- Verbs: `make setup | build | typecheck | lint | fmt | test | test-affected | integrity | verify | doctor | install | demo | play | ui`.
