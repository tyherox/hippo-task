# Schema Design v0 — Open Agent-Native Task Schema

> **Reading guide.** §0–§10 are the *design target*. **§11 is what `hippo-task 0.1.0` actually implements** — the exact event format, verbs, and rules, including a few refinements that came out of hardening. Where the two differ, §11 describes today's behaviour; everything §11 lists as deferred still has room reserved here. This schema supersedes the TypeScript prototype's (`docs/archive/typescript-v0/SCHEMA.md`); that document's 10-platform field research remains the input for adapters.

The schema *is* the product, so this is the most important artifact. Designed against three first-class constraints: **interop (always), concurrency/swarm, audit/ledger.** Everything below earns its place or it's cut.

---

## 0. The one decision that unifies all three focuses

**The Event is the primitive. The Task is a projection.**

Agents never mutate a task record in place. They **append events** to an append-only, hash-chained ledger *(superseded by §11 A: shipped append-only, not yet hash-chained — deferred)*. A task's current fields are computed by folding its events (`state = reduce(events)`). This collapses three hard problems into one mechanism:

- **Audit** is free — the ledger *is* the history (nothing is ever overwritten).
- **Concurrency** is tractable — appends don't fight; conflicts resolve deterministically at projection time (per-field last-writer-wins by clock).
- **Local-first + git** works — append-only logs merge by union; no central server.

Interop then operates on the *projection* (the Task), which is deliberately the common denominator of VTODO / OSLC / modern trackers.

Two concepts, held in the head at once (conceptual integrity): **Event → (fold) → Task.**

---

## 1. Design principles

1. **Minimum-useful, not comprehensive.** The core is the *intersection* of what real tools share, not the union. Anything vendor-specific lives in namespaced extensions, never the core (resist becoming Jira).
2. **Interop-anchored from field one.** Every core field maps 1:1 to iCalendar VTODO (RFC 5545) and Dublin Core / OSLC-CM, which in turn map to Linear/Jira/GitHub. Identity and foreign keys are reserved on day 0 even though sync ships later — so we never paint into a corner.
3. **Event-sourced.** Append-only ledger is the source of truth; task state is derived and disposable (rebuildable by replay).
4. **Local-first & mergeable.** Plain text (JSONL/JSON), git-friendly, no server, you own the files.
5. **Deterministic & replayable.** Same events → same state, on any machine, forever.

---

## 2. The primitives

### 2.1 Event (source of truth)
```jsonc
{
  "eid":       "01J9Z…",          // ULID: globally unique, time-sortable, client-generated (no coordination)
  "task":      "01J9Y…",          // the task this event applies to
  "type":      "update",          // create | update | claim | heartbeat | release | relate | note | complete | cancel
  "actor":     "agent:claude-code",// SEMANTIC who (human:<id> | agent:<name>) — for audit/display
  "node":      "n:7f3a2c",        // PHYSICAL writer instance (this window/session/machine) — the HLC tiebreak; one actor can have many nodes
  "hlc":       "1738368000123:2:n7f3a2c", // Hybrid Logical Clock: physical ms : counter : node — total causal order across all writers
  "wall":      "2026-07-31T12:00:00.123Z",     // human-readable wall clock (never used for ordering)
  "data":      { "state": "doing" },           // the delta (only the fields this event changes)
  "prev":      "sha256-…",        // hash of the previous event in THIS task's chain (tamper-evidence)
  "hash":      "sha256-…"         // sha256(canonical(event without hash))
}
```

*(superseded by §11 A: the shipped event is `{eid, task, ts, actor, node, type, data}` — `ts` is monotonic unix millis; `hlc`, `wall`, `prev` and `hash` are deferred.)*

### 2.2 Task (projection — never edited by hand; computed from events)
```jsonc
{
  "id":        "01J9Y…",          // = VTODO UID / dcterms:identifier
  "title":     "…",               // = SUMMARY / dcterms:title            [required]
  "body":      "markdown",        // = DESCRIPTION                        [optional]
  "state":     "todo",            // todo|doing|done|cancelled            [required]  (see §3 — 'blocked' is derived)
  "assignee":  "agent:codex",     // = ATTENDEE — DURABLE intent: who should own it   [optional]
  "priority":  "none",            // none|low|med|high|urgent (0–4)  = PRIORITY  [CORE]
  "labels":    ["infra"],         // = CATEGORIES                         [optional]
  "relations": [ {"type":"blockedBy","task":"01J…"} ], // = RELATED-TO    [optional]
  "lease":     {"holder":"agent:codex","expires":"…","beat":"…"} | null,  // EPHEMERAL: who's actively doing it now (derived, §4). Renamed from 'claim' to disambiguate from assignee.
  "seq":       7,                 // = SEQUENCE: count of mutating events (optimistic concurrency + sync)
  "created":   "…", "updated": "…",// = CREATED / LAST-MODIFIED (derived)
  "refs":      [ {"system":"linear","id":"ENG-142","url":"…","etag":"…","synced":"…"} ], // interop round-trip anchor
  "ext":       { "x-mycorp": { … } } // namespaced pass-through for non-core fields
}
```

*(superseded by §11 C: shipped `relations` are `{"rel":"blocked-by","task":…}`; `lease` is `{holder, node, expires_ms, active}`; timestamps are `created_ms`/`updated_ms` in unix millis; a derived `num` was added; `refs` and `ext` are reserved, not implemented.)*

**That's the whole core.** ~13 fields, every one mapping to an open standard or earning its place for concurrency/audit.

**Explicitly excluded from the v0 core** (→ `ext`, or a later appliance): due dates, estimates/points, sprints/cycles, projects/boards, custom workflows/statuses, attachments, reactions, first-class comments (comments are just `note` events). Rationale: none are common-denominator-essential; each pulls toward Jira-scale breadth.

---

## 3. Focus 1 — Interop (kept in mind at all times)

**Core = the intersection, named to map 1:1.** Mapping table (this is the contract that makes sync possible later):

| Our field | iCalendar VTODO (RFC 5545) | Dublin Core / OSLC-CM | Linear / Jira / GitHub |
|---|---|---|---|
| `id` | `UID` | `dcterms:identifier` | issue id / key |
| `title` | `SUMMARY` | `dcterms:title` | title / summary |
| `body` | `DESCRIPTION` | `dcterms:description` | description |
| `state` | `STATUS` (NEEDS-ACTION/IN-PROCESS/COMPLETED/CANCELLED) | `oslc_cm:status` | status/state |
| `assignee` | `ATTENDEE` | `dcterms:contributor` | assignee |
| `priority` | `PRIORITY` (0–9) | — | priority |
| `labels` | `CATEGORIES` | `dcterms:subject` | labels |
| `relations` | `RELATED-TO` (PARENT/CHILD/…) | `oslc_cm:relatedChangeRequest` | parent/blocks/relates |
| `seq` | `SEQUENCE` | — | (etag/version) |
| `created`/`updated` | `CREATED`/`LAST-MODIFIED` | `dcterms:created`/`modified` | createdAt/updatedAt |

Interop mechanisms designed in **now**, even though sync is deferred:
- **`refs[]` (foreign-key anchor).** `{system, id, url, etag, synced}` — the thing that makes round-trips lossless. The #1 reason migrations fail is losing the mapping between "our record" and "their record"; we reserve it on day 0.
- **`ext` (namespaced pass-through).** Vendor/custom fields ride along opaquely (like VTODO `X-` properties / Jira custom fields) without polluting the common core. Interop rule: **carry unknown fields through untouched; never drop them.**
- **Canonical enums with documented mappings.** `state` and `priority` are tiny closed sets that map onto each system's superset; the mapping table is versioned with the schema.
- **Round-trip fidelity requires stashing native values (self-review fix).** Mapping a rich vendor status (Jira "In Review") down to our canonical `state:doing` *loses information* on the way back. So when a task originates from or syncs to an external system, keep its **native field values verbatim in `refs[].native` / `ext`**, and restore from those on export. The canonical core is for *reasoning*; the native stash is for *lossless return*. (This is the exact "a Deal ≠ an Opportunity" failure from the CRM research — solved by never discarding the source's own values.)
- **Interop-driven design change (caught in design review):** "blocked" is **NOT a top-level state** — it doesn't exist in VTODO's status set and wouldn't round-trip. Instead, *blocked is derived*: a task is blocked iff it has ≥1 `blockedBy` relation whose target isn't `done` *(superseded by §11 D3: the target must be **open** — a cancelled blocker no longer blocks)*. This keeps `state` interop-clean and makes blockage a computed truth, not a hand-set flag that drifts.

---

## 4. Focus 2 — Concurrency & swarm mechanics

**Identity (no coordination needed).** IDs are **ULIDs** (or UUIDv7): 128-bit, timestamp-prefixed, lexicographically sortable, generated client-side. Two agents can create tasks at the same instant with zero collision and no server. (Optional human-facing short id like `ENG-142` is a derived label, not identity.)

**Ordering across agents.** Every event carries a **Hybrid Logical Clock** (`physical_ms : counter : node`). HLC gives a total, causally-consistent order without a central clock and stays close to wall time (unlike pure Lamport). *Fallback if we want simpler:* Lamport counter + actor-id tiebreak.

**Conflict resolution — two rules by field kind (self-review fix):**
- **Scalar fields** (`state`, `title`, `assignee`, `priority`, `body`) → **last-writer-wins by HLC**. Deterministic; a tiny LWW-register per field, no CRDT library.
- **Collection fields** (`labels`, `relations`) → **add/remove ops, not whole-array LWW.** Emit `relate`/`unrelate` and `label-add`/`label-remove` events; the projection folds them as a set. *Why this matters:* whole-array LWW would silently clobber a concurrent label add by the other agent (two agents each add a label → one vanishes). Set-ops make concurrent collection edits commute. This is the one place naïve LWW is a real bug — caught in review.

No operational transforms, no merge UI either way.

**Swarm claim/lease (the "who works on this" problem).** To stop two agents doing the same task:
- `claim {actor, expires, beat}` event takes a **lease** on a task.
- An agent must not start a task whose lease is unexpired and held by someone else.
- `heartbeat` events extend the lease; a crashed agent's lease **expires** and the task becomes reclaimable (self-healing, no lock server).
- Contended claim resolves by HLC precedence: first valid claim wins; the loser sees `claim` occupied on next read. All of it is in the ledger (auditable).

**Git-mergeability (why local-first works).** Events are immutable and append-only, so merging two branches = **union of events, re-sorted by HLC**, then re-project. Two agents editing *different* tasks never conflict. Two agents editing the *same* task produce concatenated events that fold deterministically.
- *Storage choice (surfaced for the spike):* **event-per-file** (`.hippotask/<id>/<eid>.json`) = zero git conflicts ever (merges are pure additions), at the cost of many small files. **Append-log** (`.hippotask/<id>/events.jsonl`) = readable, but same-task concurrent appends can produce a line-level git conflict (mechanically resolvable by union). Recommend event-per-file for the swarm case; keep a compacted `events.jsonl` as a derived, regenerable view.

---

## 5. Focus 3 — Audit / ledger

The ledger isn't a feature bolted on — it's the source of truth (§0), so audit is total by construction.

- **Immutable & append-only.** Events are never edited or deleted. A "change of mind" is a new event; an "undo" is a **compensating event** (append the inverse), never a rewrite.
- **Tamper-evidence (hash chain).** Each event stores `prev` = hash of the previous event *in that task's chain* and `hash` = sha256 of its own canonical form. Altering history breaks the chain and is detectable. *Per-task* chains (not one global chain) are chosen deliberately: they keep local-first/git merges conflict-free while still proving each task's history is intact.
- **Replay & time-travel.** State at any moment = fold events up to that HLC. Enables blame ("which agent set `state=done`, when, citing what"), audit export, and safe rebuilds.
- **Activity verbs aligned to ActivityStreams 2.0** (Create/Update/Add/Remove/Assign/Complete) so the *history itself* is interop-friendly, not just the task.
- **Comments/notes are first-class events** (`note`), not a separate table — one mechanism, fully audited.

---

## 6. Worked example (two agents, one task, concurrent)

```jsonl
{"eid":"…01","task":"T1","type":"create","actor":"human:ana","hlc":"1000:0:ana","data":{"title":"Ship auth","state":"todo"},"prev":null,"hash":"h1"}
{"eid":"…02","task":"T1","type":"claim","actor":"agent:claude-code","hlc":"1005:0:cc","data":{"expires":"…+10m"},"prev":"h1","hash":"h2"}
{"eid":"…03","task":"T1","type":"claim","actor":"agent:codex","hlc":"1005:0:cx","data":{"expires":"…+10m"},"prev":"h1","hash":"h2b"}  // concurrent claim!
```
Projection resolves the contended claim by HLC tiebreak (`cc` vs `cx` at equal physical time → deterministic actor order) → **claude-code holds the lease; codex reads "claimed" and picks another task.** No lock server, fully auditable, and it merged from two branches by union.

---

## 7. Open questions to validate in real use (don't pre-decide)
- **HLC vs Lamport** — HLC recommended; confirm the wall-clock benefit is worth the complexity for a single-user-first tool.
- **Per-task hash chain granularity** — per-task (chosen) vs per-actor vs global; revisit if global integrity proof becomes a requirement.
- **Storage layout** — event-per-file (zero-conflict) vs append-log (readable); measure the file-count cost in a real repo.
- **Agent invocation surface** — CLI vs direct file read/write vs an **MCP tool** wrapper (likely all three eventually; which first?).
- **Minimum viable field set** — is `priority` even core for v0, or does it belong in `ext` until a real need appears? (Kano-test each field against actual use.)

---

## 8. Self-review findings (adversarial pass — kept honest)

Two fixes already folded into §3/§4 above:
- **Collection fields need set-ops, not array LWW** (§4) — else concurrent label/relation adds silently clobber. Real bug, fixed.
- **Round-trip needs a native-value stash** (§3) — canonical mapping alone loses vendor detail on the return trip. Fixed.

Remaining flags (not blockers; decide during the spike):
- **Two "who" fields — `assignee` vs `claim`.** Kept distinct on purpose: `assignee` = durable *intent/routing* (may be a human or "any agent"); `claim` = ephemeral *execution lease* by a specific agent. Watch this as a conceptual-integrity risk — if it confuses in real use, collapse or rename. It does *not* add a third top-level concept (claim is expressed as events).
- **Ledger growth / compaction (real gap).** Append-only logs grow forever. Needs a **snapshot + compaction** story: periodically write a signed state snapshot and keep only events since it (preserving a hash-chain anchor). Deferred, but must exist before heavy use.
- **HLC assumes roughly-synced clocks.** A badly-skewed client clock can win LWW unfairly. Acceptable single-user-first; revisit if multi-machine skew bites.
- **Anti-overengineering staging (M2 — the most important flag).** The **field set + event-sourcing is the v0 commitment**; the *advanced* machinery (full HLC, hash-chaining, expiring leases) is earned by real multi-agent contention, not built up front. Stage it: start with wall-clock + actor tiebreak + a simple claim flag and a single `events.jsonl`; add HLC, hash-chain, and event-per-file **when two agents actually contend**. The schema is designed to *accommodate* all of it without a breaking change — that's the point of reserving the fields now — but don't build the whole engine before the pain is real.

## 9. Why this satisfies the brief
- **Interop:** core = the real VTODO/OSLC/tracker intersection; `refs` + `ext` reserve lossless round-trips from day 0; "blocked-as-derived" is an interop-forced correction.
- **Concurrency/swarm:** ULID identity + HLC order + per-field LWW + expiring claim/lease = deterministic multi-agent writes with no server, mergeable by git.
- **Audit/ledger:** event-sourced, immutable, hash-chained, replayable — audit is the substrate, not a feature.
- **Minimum-useful:** ~13 core fields + 9 event verbs *(superseded by §11 B: 14 verbs shipped — `update` split per field, plus the collection ops)*; everything else is `ext` or deferred.

---

## 10. v0.2 — Decisions locked + additions (from design review)

**Decision log:** HLC over Lamport ✓ · per-task hash-DAG ✓ · single-file storage behind an adapter ✓ · CLI-first ✓ · `priority` is **core** ✓ · `claim` → `lease` rename ✓ · compaction from day 0, user-configurable ✓ *(superseded by §11 E: compaction and `snapshot.json` are deferred)*.

### A. Identity — `actor` vs `node` (robust for many concurrent writers)
Two ideas, cleanly separated:
- **`actor`** = *semantic* who, for audit/display (`human:ana`, `agent:claude-code`). One actor can have many concurrent sessions.
- **`node`** = *physical* writer instance — one per process that appends: a Claude Code **window**, a Codex session, a machine daemon. Generated fresh per session, stable for that session. **`node` is the HLC tiebreak.**

Why (your point): the swarm isn't only Codex-vs-Claude. **Two Claude Code windows on the same repo are two nodes** → distinct HLC tiebreaks → their events never collide or misorder, even though `actor` is identical. The same mechanism scales to multiple machines. **Rule: a unique `node` id per concurrent writer, always.**

### B. Storage — single-file default behind a `StorageAdapter` (extensible)
Default (cheapest, local, corruption-solid): a single append-only **`ledger.jsonl`** + a `snapshot.json`.
- **Corruption resilience:** one complete JSON event per line; appends are `O_APPEND` + `flock` + `fsync` so concurrent local processes serialize and never interleave; a torn trailing line is invalid JSON → detected and skipped/repaired on load; snapshots written via temp-file + atomic rename.
- **Swappable without touching the schema:**
  ```
  StorageAdapter:  append(event) · readSince(hlc) -> events · putSnapshot(s) · getLatestSnapshot()
  ```
  Default = SingleFileAdapter. Future optional adapters (your "alternative storage"): EventPerFile (zero-conflict git), SQLite (indexed/large repos), SyncServer/CRDT-backend (multi-machine). The schema, HLC, fold, and hashing are all adapter-agnostic.

### C. Compaction & snapshots (bounded growth, day 0, user-configurable) *(superseded by §11 E: not in 0.1.0)*
- A **snapshot** = folded state at an HLC + integrity anchor: `{through_hlc, state, events_hash, prev_snapshot_hash, created}`.
- **Config:** `compaction: { every: "1w" | "1m" | "<N> events" }` (default weekly) — the user sets the period, per your suggestion.
- Hot path: `state = latest_snapshot + fold(events_since_snapshot)` → **projection cost is bounded**; you never re-fold full history.
- **Audit preserved:** older events roll into a compressed `archive/`, not deleted (unless an explicit retention policy prunes them). Snapshots hash-anchor to the events they summarize, so full history is still provable/replayable from the archive.

### D. Per-task hash — performance
- Hash computed **once per append** (sha-256 over canonical JSON — microseconds). Per-task DAG ⇒ integrity checks are **O(events in that task)**, not global.
- Snapshots carry `events_hash`, so routine verification only spans events since the last snapshot (full-history verification available, rarely needed).
- **Locked spec rule:** canonical serialization (stable key order, normalized numbers) so hashes reproduce across machines/languages.
- **§5 correction (from the worked example):** under concurrency the per-task structure is a **hash-linked DAG (git-like)** — it forks on concurrent appends and re-converges — not a linear chain. Tamper-evidence is unchanged.

### E. Multi-machine readiness — answering "is our architecture suitable?"
**Yes, and it's the good news:** the *data model* is already a distributed-systems design, which is the hard part. Event-sourcing + HLC + per-field LWW + set-op collections + globally-unique ULIDs + per-writer `node` identity is *exactly* what multi-machine sync needs. The only multi-machine-specific work is the **transport/storage layer**, which is deliberately behind the adapter (B): local single-file today; a sync-server or CRDT-backend adapter tomorrow — **no schema change required.**

What we lock now to keep that path open (all already true): globally-unique ids (ULID), unique `node` per writer (A), monotonic causal clock (HLC), no single-writer assumption (event log), storage behind an adapter (B). So the "robust, scalable, extensible" goal is satisfied *by construction* — multi-machine becomes an adapter, not a rewrite.

### F. Priority is core (confirmed)
`priority ∈ {none, low, med, high, urgent}` (or 0–4), mapping to VTODO `PRIORITY` (0–9) and Linear/Jira/Monday/ClickUp. It's in every modern tool → it earns core (not `ext`).

---

## 11. As shipped — `hippo-task 0.1.0` (the implemented contract)

The staging rule in action: the field set + event sourcing shipped; the heavy machinery waits for real contention. Source of truth for everything below: `src/` and its tests (`make verify`). Changes are logged in `CHANGELOG.md`.

### A. Event format (frozen — a test fails if it changes)
```jsonc
{"eid":"01K…","task":"01K…","ts":1790334150017,"actor":"agent:claude","node":"cc","type":"label-add","data":{"label":"x"}}
```
- `ts` is unix millis, **strictly increasing within a ledger**: written as `max(wall clock, newest ts + 1)` under the file lock. That's the physical half of the HLC (§4) — enough for one machine, where ledger order now equals time order. The logical counter and cross-machine receive rule arrive with sync.
- Not yet on the event: `hlc`, `wall`, `prev`, `hash` (full HLC and the per-task hash-DAG are deferred). Adding them later is additive.
- Unit verbs (`release`, `complete`) carry no `data`.

### B. Verbs as shipped (14) vs the §2.1 design verbs
| Design verb | Shipped as |
|---|---|
| `create` | `create` `{title, priority, body, assignee}` |
| `update` | one verb per field: `set-title`, `set-state`, `set-priority`, `set-assignee`, `set-body` (per-field LWW falls out naturally) |
| collection ops (§4) | `label-add`, `label-remove`, `relate`, `unrelate` |
| `claim` | `lease` `{holder, expires_ms}` (the §10 rename) |
| `heartbeat` | a `lease` by the *same worker* renews it |
| `release`, `note`, `complete` | same names |
| `cancel` | `set-state` → `cancelled` |

### C. Task projection as shipped (JSON: `hippo-task show --json`)
`id`, `num`, `title`, `body`, `state`, `priority`, `assignee`, `labels`, `relations` (`{"rel":"blocked-by","task":…}` — kebab-case, not the `blockedBy` of §2.2), `blocked`, `lease` (`{holder, node, expires_ms, active}`), `created_ms`, `updated_ms`, `seq`. Not yet implemented: `refs`, `ext` (still reserved). `num` is a **derived** friendly handle (`#3`, creation order) — stable on one machine, but a future multi-machine merge may renumber it; the ULID `id` is the permanent identity.

### D. Rules decided while hardening (refinements of §3–§5)
1. **A lease belongs to actor + node** (refines §10A). `node` isn't only the tiebreak: two windows of the same agent are two *workers*, and only the holding worker can renew or release. Without this, two Claude windows could both "hold" one task — the first real use case.
2. **Closed tasks hold no lease.** Entering done/cancelled clears the lease; a lease on a closed task is rejected.
3. **Blocked = has a blocked-by target that is still *open*** (refines §3's "isn't done"): a cancelled blocker no longer blocks; missing or self references never block.
4. **Idempotent projection.** An event that changes nothing is still appended (audit) but doesn't bump `seq`/`updated_ms`; the fold reports it, and history shows it as `applied: false` — `(rejected)` for a refused lease, `(no change)` otherwise.
5. **A note is activity**, not a content change: it bumps `updated_ms`, not `seq`.
6. **First `create` wins**; events for a task that doesn't exist (yet, in fold order) are no-ops.
7. **Physics vs etiquette.** Rules 1–6 live in the fold and hold for *any* ledger in *any* order (a property test checks order-independence over random ledgers). CLI policy lives above it: while a lease is active only its holder changes the task's state or closes it (`--force` overrides), agents must name their node, no self-blocking, no empty text.

### E. Storage as shipped (§10B, first slice)
Single `.hippotask/ledger.jsonl`. Writers: exclusive `flock` (10 s timeout → `io` error, never a hang) → read → decide → one `write_all` → `fsync` (rolled back to the previous length if either fails; a newly created file's directory is flushed too). Readers: shared lock. A torn last line is skipped with a warning and fenced off before the next append. Not yet: `snapshot.json`, compaction, other adapters.

### F. Interfaces
CLI with a stable exit-code contract (0 ok · 1 io · 2 usage · 3 not_found · 4 conflict) and `--json` on every command (shapes in `AGENTS.md`). Privacy by default: without `HIPPO_ACTOR`, events record `human:local` — no username or hostname.

### G. The §7 open questions — status after 0.1.0
- HLC vs Lamport → HLC-lite shipped (monotonic physical ms); full HLC waits for multi-machine sync.
- Hash-chain granularity → still open (deferred).
- Storage layout → single append-log shipped; event-per-file still an option behind the adapter.
- Agent surface → CLI + `--json` first; an MCP wrapper is the natural next surface if real use earns it.
- Minimum field set → unchanged; real use decides.
