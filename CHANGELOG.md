# Changelog

Notable changes to HippoTask (`hippo-task`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer (while 0.x, a minor bump may break things — it will say so here).

## [Unreleased]

### Added

- MIT license (`LICENSE`, and `license = "MIT"` in `Cargo.toml`).
- `list --ready`: the tasks someone could pick up right now — open, not blocked, and no active lease, including `doing` work whose lease ran out. AGENTS.md's pick step uses it ([ADR-002](docs/decisions/adr-002-released-work.md)).
- `release --json` includes `released`: true if you held the lease and gave it back, false if there was nothing of yours to release. Additive — the task object is unchanged.
- `make play` — the playground's interactive REPL (`playground/play.sh`) as a Makefile verb.
- `make doctor` warns when `python3` is missing (`make ui` needs it).
- Tests: the `--json` shapes (task, lease, event) are frozen by a CLI test, the way the on-disk event already was; closed-task transitions are pinned (`update --state todo` reopens, `done` on a done task is a recorded no-op, `lease`/`start` on a closed task is exit 4, a forced state change leaves the other worker's lease in place).

### Changed

- `release` by the holder returns a started (`doing`) task to `todo`, so unfinished work goes back in the queue instead of sitting in `doing` with nobody on it. The fold and the event format are unchanged, and a release by anyone else still changes nothing ([ADR-002](docs/decisions/adr-002-released-work.md)).
- Playground scripts (`demo.sh`, `play.sh`, `serve.py`) build with `--locked`, like the Makefile.
- The playground UI's Live mode reads lease activity from the CLI's `lease.active` instead of the browser clock.

### Fixed

- CI: the test-integrity gate no longer passes vacuously on `workflow_dispatch` (empty base) or on a branch's first push (all-zero base); it falls back to a usable base commit.
- Docs realigned with the shipped model: README and AGENTS.md say exactly when a closed task is a conflict (`lease` / `start` only), give the lease range (1–1440 minutes, default 10), and describe the lints, the integrity gate, and CI as they are.

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
