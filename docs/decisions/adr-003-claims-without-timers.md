# ADR-003 — Claims without timers: whoever knows hands the work back

**Status:** Accepted · **Date:** 2026-09-26

## Context
A lease does two jobs. **Exclusion:** someone is on this task, so don't take it. **Failure detection:** that someone is gone, so the task can move on. 0.1.x does both with a timer: a lease expires unless renewed, so a crashed worker's task frees itself.

Timers suit processes that can renew on a clock. Coding agents can't. They act in turns, one long test run looks exactly like a crash, and nobody knows up front how long a task will take. So every claim forces a guess, and every guess is wrong one way or the other: too short, and a live agent's work is taken over; too long, and a crashed agent's task stays locked for hours.

A day of real use (47 agent workers, about 1,000 events) bore this out:
- Agents took the 10-minute default and extended it within seconds, to 60–120 minutes. In the first run, the tasks behind the two-hour leases took a median of 14 minutes.
- The timers never did their job: no lease expired and handed a task to another worker.
- They still cost something: one event in six was lease bookkeeping, and half of those were renewals.
- The parties that *do* know a worker has failed — the orchestrator that launched it, the person who closed its window, the worker itself on a clean exit — had no way to say so except to wait out a timer.

## Decision
1. **A claim holds until it's released or the task closes.** `start` records a new `claim {holder}` event with no expiry. Otherwise it follows the lease rules: it belongs to actor + node, a claim by the current holder changes nothing, and closing the task clears it.
2. **Whoever knows hands the work back — explicitly, and on the record.**
   - **The worker:** `hippo-task release --all` gives back everything it holds, each `doing` task returning to `todo` (ADR-002). Run it from a session-end hook or a workflow's teardown.
   - **A person or an orchestrator:** `hippo-task reclaim 3 --reason "…"`, or `reclaim --from <node>` for everything held on that worker's node (`--from`, because `--node` already names the node you act from). The new `reclaim {holder, node, reason}` event names the claim it takes back, so it changes nothing if that claim is already gone: two reclaims, or a reclaim racing a release, can't clobber a newer claim. The task returns to `todo`.
   - **Who may (etiquette, `src/ops.rs`):** human actors may reclaim; an agent needs `--force`, which AGENTS.md reserves for the process that launched the worker. Actor names are self-declared, so this is recorded etiquette, not security.
3. **Staleness is shown, not enforced.** Every event the holder writes on its task counts as a sign of life, so the task's hold gains `since_ms` and `last_seen_ms`, and `list --held` shows who holds what and how long each has been quiet. A person or an orchestrator decides what "too quiet" means.
4. **Retired:** the `lease` command and `--minutes`. Reserving a task without starting it is what `assignee` is for, and there's no timer left to set. Existing `lease` events keep folding exactly as before — timed — so every existing ledger reads the same.

## Alternatives
- **Keep timers; teach agents to size them and renew with every note.** Rejected: still a guess per claim, and a long tool call still looks like death.
- **An idle timeout measured from the holder's last activity.** Deferred: better than a fixed term, but still a clock deciding that a worker is dead. It's the natural rule for automatic takeover, if that's ever needed.
- **Detect liveness from the worker's process.** Rejected: the CLI runs as a short process per command, the agent's session process isn't reliably identifiable across tools, and it would record details about the machine (privacy by default).
- **Let agents take over quiet claims themselves.** Deferred until an unsupervised crash actually happens — none so far — so that a slow but live worker's task is never taken.

## Consequences
- The model gains two event kinds, `claim` and `reclaim`; existing kinds keep their frozen shape (`on_disk_event_shape_is_frozen`). The fold's order-independence property test must cover both, with two new invariants: closed tasks hold nothing, and a reclaim naming anyone but the current holder changes nothing.
- The task JSON keeps its `lease` object, which gains `since_ms` and `last_seen_ms`; its `expires_ms` is `null` for a claim. Together with retiring `lease` and `--minutes`, this is a breaking change, so it ships as **0.2.0** and the CHANGELOG says so.
- A 0.1.x binary skips `claim` and `reclaim` lines with a warning, so everything that shares a ledger must upgrade together.
- A worker that crashes with no one supervising it holds its task until a person reclaims it; `list --held` is how they notice. Accepted: every worker so far had a supervisor — an orchestrator or a person.
- A person can finally free a crashed worker's task. Until now `--force` changed only the state and left the dead worker's lease in place (pinned by `a_forced_state_change_leaves_the_holders_lease_intact`).
- AGENTS.md's loop loses its renew step and gains one line: on exit, `hippo-task release --all`.
