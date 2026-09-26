# ADR-002 — Released and abandoned work goes back in the queue

**Status:** Accepted · **Date:** 2026-09-26

## Context
In real multi-agent use, an agent released a task it couldn't finish. `release` cleared the lease but left the state `doing`, so the task vanished from the documented pick query (`list --state todo`) until someone reset it by hand 15 minutes later.

The cause is an asymmetry: `start` is a lease *plus* `set-state doing`, but `release` undid only the lease. An expired lease — a crashed agent — ends the same way, and the fold can't change state when a lease expires: time passing isn't an event. So the design's promise that a crashed agent's task "becomes reclaimable" (schema-design §4) held for `start`, but no agent following the documented pick step would ever see the task.

## Decision
1. **`release` undoes `start`.** When the holder releases a `doing` task, one transaction records `set-state todo` — while the holder still holds the lease, so the state change is theirs to make — and then `release`. A task that was only leased keeps its state; a release by anyone else still changes nothing.
2. **`list --ready`** lists what can be picked up right now: open, not blocked, and no active lease. That includes `doing` tasks whose lease expired, or that were released before this change. AGENTS.md's pick step uses it.

Both are CLI policy (`src/ops.rs`). The fold and the on-disk event format are unchanged, so every existing ledger folds exactly as before.

## Alternatives
- **Reset the state in the fold's `Release` rule.** Rejected: it would change what existing ledgers fold to, and it still couldn't cover expiry.
- **A derived `stale` flag on the task**, like `blocked`. Deferred: `--ready` answers the pick question without a new JSON field. Revisit if agents need to see abandoned work anywhere else.

## Consequences
- `tests/cli.rs` pinned "release doesn't touch the state"; that expectation flips to `todo` (marked `test-weaken-ok`, reviewed by the maintainer).
- The old pick step (`list --state todo`) now sees released work, but only `--ready` also finds expired `doing` tasks. Projects that copied AGENTS.md §A should refresh it.
- `start` on an abandoned `doing` task records a `set-state doing` that changes nothing; its history shows "(no change)".
