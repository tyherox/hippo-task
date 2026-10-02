# Changelog

Notable changes to HippoTask (`hippo-task`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer (while 0.x, a minor bump may break things — it will say so here).

## [0.4.1] - 2026-10-02 — internal release

Portable: install and run it on macOS, Linux or Windows, with no Rust needed. Nothing changes for agents — same commands, same JSON.

### Added

- Prebuilt binaries for every release, each with a SHA-256 checksum: macOS (Apple silicon and Intel), Linux (x86_64 and arm64, statically linked, so they run on any distribution) and Windows (x86_64). Pushing a version tag builds and publishes them, then installs them on each OS as a check.
- `scripts/install.sh` installs the right build on macOS or Linux with one line, checks its checksum, and upgrades in place when run again.
- CI runs on Windows too: clippy and the whole test suite.

### Fixed

- Windows: creating a store no longer warns that its folder couldn't be flushed to disk. Windows can't flush a folder; the ledger file itself is still flushed on every write.
- Windows: a typed `~/` (or `~\`) at the `init` prompt means your profile folder, and paths show as `C:\…` rather than `\\?\C:\…`.

## [0.4.0] - 2026-09-27 — internal release

Where tasks live: chosen once, found from anywhere ([ADR-005](docs/decisions/adr-005-where-tasks-live.md)). **This release breaks things:** commands no longer create a store by accident.

### Breaking

- Outside a project that has a store, commands exit 2 and ask for `hippo-task init` instead of creating `.hippotask/` in the current folder. Existing `.hippotask/` folders keep working with no setup. `--dir` / `HIPPO_DIR` still name a project explicitly and, as before, create its `.hippotask/` on the first write.

### Added

- `hippo-task init` chooses where a project's tasks live: in the project (`.hippotask/`, the default) or in another folder (`--folder`), which the project points to from `.hippotask/store.json`. At a terminal it asks; otherwise pass `--here` or `--folder`. It sets up the enclosing repository's root when run from a subfolder, and never moves an existing ledger.
- Inside a git repository, `init` keeps the tasks out of git by default with a `.gitignore` in the store folder — never touching the repository's own — and `--keep-in-git` lets them be committed. A pointer is always kept out of git, since it names a path on this machine.
- Every command finds the project's store from the current folder upward, the way git finds `.git`, without climbing out of a repository. An agent working in a subfolder no longer starts a second ledger there.
- `hippo-task guide` prints the agent protocol from the installed binary, so a project's AGENTS.md only has to point to it. `init` suggests that line, an identity per agent window, and a Claude Code session-end hook that runs `hippo-task release --all`.
- The pointer carries a `kind` (`local` today), so a hosted store can be added later; a store of a kind this version can't open is a clear error.

### Fixed

- A closed task is never `blocked`. Before, a cancelled or done task still showed as blocked while its blocker was open.

## [0.3.0] - 2026-09-27 — internal release

Duplicates: find before filing, mark when found ([ADR-004](docs/decisions/adr-004-duplicates.md)). **Upgrade every tool that shares a ledger together:** 0.2.x can't read `duplicate-of` relations and skips those lines with a warning.

### Added

- `list --search <text>`: tasks whose title or description contains the text, ignoring case, in any state. Repeat it to require several terms. AGENTS.md now asks agents to search before they `add`.
- `update <id> --duplicate-of <id>`: links a duplicate to its original and cancels it in one step, so it never counts as finished work. Closing etiquette applies; a task can't be a duplicate of itself, and `--state` can't be combined with it.
- The relation `duplicate-of` in the task JSON's `relations`. Only `blocked-by` affects `blocked`.

## [0.2.0] - 2026-09-26 — internal release

Claims replace timed leases ([ADR-003](docs/decisions/adr-003-claims-without-timers.md)). **This release breaks things — upgrade every tool that shares a ledger together:** 0.1.x skips `claim` and `reclaim` lines with a warning.

### Breaking

- `start` records a **claim, with no timer**: the task is yours until you finish or release it, or someone reclaims it. Timed leases already in a ledger keep expiring as they did.
- The `lease` command and `--minutes` are retired. `hippo-task lease` now exits 2 with a pointer to `start`; `start --minutes` is an unknown flag (exit 2).
- In the task JSON's `lease` object, `expires_ms` is `null` for a claim.

### Added

- `release --all`: give back everything this worker (actor + node) holds — for a session's exit hook or a workflow's cleanup.
- `reclaim <id>` and `reclaim --from <node>`, with `--reason` and `--force`: a person — or, with `--force`, the orchestrator that launched a worker — takes back tasks a worker can't give back. The history records who took them back, from whom, and why; a reclaim naming a worker that no longer holds the task changes nothing. `release --all` and `reclaim` print arrays of task objects.
- `list --held`, and `since_ms` / `last_seen_ms` on the `lease` object: who holds what, and when each holder was last seen (any event it writes on its task counts).
- Event kinds `claim` and `reclaim`; the existing kinds keep their frozen shape.
- MIT license (`LICENSE`, and `license = "MIT"` in `Cargo.toml`).
- `list --ready`: the tasks someone could pick up right now — open, not blocked, and held by nobody, including `doing` work whose 0.1.x lease ran out. AGENTS.md's pick step uses it ([ADR-002](docs/decisions/adr-002-released-work.md)).
- `release --json` includes `released`: true if you held the task and gave it back, false if there was nothing of yours to release. Additive — the task object is unchanged.
- `make play` — the playground's interactive REPL (`playground/play.sh`) as a Makefile verb.
- `make doctor` warns when `python3` is missing (`make ui` needs it).
- Tests: the `--json` shapes (task, lease, event) are frozen by a CLI test, the way the on-disk event already was; closed-task transitions are pinned (`update --state todo` reopens, `done` on a done task is a recorded no-op, `start` on a closed task is exit 4, a forced state change leaves the other worker's claim in place).

### Changed

- `release` by the holder returns a started (`doing`) task to `todo`, so unfinished work goes back in the queue instead of sitting in `doing` with nobody on it. The fold and the event format are unchanged, and a release by anyone else still changes nothing ([ADR-002](docs/decisions/adr-002-released-work.md)).
- Playground scripts (`demo.sh`, `play.sh`, `serve.py`) build with `--locked`, like the Makefile.
- The playground UI's Live mode reads lease activity from the CLI's `lease.active` instead of the browser clock.
- Conflict messages say who holds a task and how long they've been quiet — `held by agent:codex@cx (quiet 7m)` — instead of a lease's time left; `list` shows a claim as `held:<worker>`.

### Fixed

- A person can now free a crashed worker's task (`reclaim`). Before, `--force` only changed the task's state and left the dead worker's lease in place until it expired.

- CI: the test-integrity gate no longer passes vacuously on `workflow_dispatch` (empty base) or on a branch's first push (all-zero base); it falls back to a usable base commit.
- Docs realigned with the shipped model: README and AGENTS.md say exactly when a closed task is a conflict, and describe the lints, the integrity gate, and CI as they are.

## [0.1.0] - 2026-09-25 — internal release

The first version meant for daily use: one human plus several coding agents, on one machine.

### HippoTask's new core

- **This Rust CLI + library is now HippoTask's core**, replacing the TypeScript prototype (kept at tag `v0-typescript`; its docs are in `docs/archive/typescript-v0/`).
- Renamed from the trial CLI `tasks`: the command is `hippo-task`, the variables are `HIPPO_DIR` / `HIPPO_ACTOR` / `HIPPO_NODE`, and the ledger lives at `.hippotask/ledger.jsonl`. To keep a 0.0.1 ledger: `mkdir -p .hippotask && mv .tasks/ledger.jsonl .hippotask/`.

### Fixed — bugs found by probing 0.0.1 before release

- **Labels passed to `add` were silently lost** — 28 of 40 in a probe. The create and its labels were stamped in the same millisecond, so a label could sort before its task existed and be dropped. Timestamps are now strictly increasing within a ledger.
- **A lease taken right after a create could vanish** the same way (across processes).
- **A torn last line — a crash mid-write — swallowed the next event.** Writers now fence it off first.
- **Two windows of the same agent could both hold one lease** (the holder was only the actor). A lease now belongs to actor + node.
- **Panics:** an invalid-UTF-8 line made the whole ledger unreadable; `hippo-task list | head` crashed on the closed pipe; a huge `--minutes` overflowed. Now: a warning, a clean exit, and a usage error.
- `show 12` could show a different task (digits fell through to id-suffix matching).
- `update --block 99` created a relation to a task that doesn't exist.
- A mistyped `--dir` silently created a new, empty task list.
- Every error exited 0.
- Your OS username was written into every event by default. See *Changed*.

### Fixed — found by an independent (different-model) review before release

- **Agents that didn't set `HIPPO_NODE` all shared the node `local`**, so two windows of one agent were the same worker and the lease stopped protecting them. Agent actors must now name their node (exit 2 otherwise).
- **Any worker could change the state of a task someone else held** (e.g. reset `doing` → `todo`); only closing was guarded. Now only the holder changes a leased task's state.
- **A write whose fsync failed was reported as failed but stayed in the ledger**, so a retry could duplicate it. Failed writes are now rolled back.
- **The first write in a new folder didn't flush the new directory entry.** It now does (best effort, with a warning if the filesystem can't).
- `add --body "  "` silently dropped the description while `update --body "  "` refused it; both refuse now. Simulate mode in the playground now applies the same input rules as the CLI.

### Added

- `--json` on every command; in that mode, warnings and errors are JSON lines on stderr.
- An exit-code contract: 0 ok · 1 io · 2 usage · 3 not_found · 4 conflict.
- `update --unblock`, `update --unassign`, `update --force`, `done --force`.
- The history marks what had no effect: `(rejected)`, `(no change)`; `show --json` has `applied`.
- Serialized, durable writes: an advisory file lock (10 s timeout), fsync before success, a torn-tail fence.
- Tests: unit and property tests for the fold, plus store, ops, CLI-contract, privacy, and docs suites. `make verify` and GitHub Actions CI (`make verify` on Linux + macOS, plus the tests on the minimum Rust, 1.89); clippy deny lints that keep panics out of product code; a test-integrity gate.
- `Makefile` verb contract, `make doctor`, `AGENTS.md`, this changelog.

### Changed

- **Privacy:** the default actor is `human:local` (was `human:$USER`). Nothing about you or your machine is recorded unless you set `HIPPO_ACTOR`.
- A lease belongs to **actor + node**; renew or release it from the same node.
- **Closed tasks never hold a lease:** completing or cancelling clears it, and leasing a closed task is refused (exit 4).
- **Only the lease holder can change a leased task's state or close it** — `--force` overrides. Agents (`agent:…`) must set `HIPPO_NODE`.
- A cancelled blocker no longer blocks; only open blockers do.
- Ids: digits always mean the task number; id suffixes need 4+ characters and ignore case.
- Text output: states and priorities print lowercase (as you type them); `add` prints `added #N <id>`; workers print as `actor@node`; times are labelled UTC; `show` names blockers (`blocked-by #2 "Write docs" (todo)`).
- "Back off" and other refusals go to **stderr with exit 4** (were stdout, exit 0).
- `--dir` must already exist; read-only commands never create files.
- Titles and labels are trimmed; empty values are refused.

### Unchanged

- The on-disk event format: 0.0.1 ledgers read exactly as before (the format is now frozen by a test). Events written by 0.0.1 keep their original order — including the old same-millisecond ties.

## [0.0.1] - 2026-08-17

- Trial CLI `tasks`: single-file event ledger, fold to state, lease coordination, derived blocked.
