# HippoTask — Provider Scorecard Rubric

> **The deterministic scoring rubric used to produce [PROVIDER_SCORECARD.md](PROVIDER_SCORECARD.md).**
>
> Given the same evidence, two auditors applying this rubric should arrive at the same score. If they disagree, the rubric is ambiguous and needs a version bump — not a judgment call.
>
> **Rubric version:** 2.1.0 · **Effective:** 2026-04-19

---

## Table of Contents

- [1. How to Use This Rubric](#1-how-to-use-this-rubric)
- [2. The 8 Dimensions](#2-the-8-dimensions)
- [3. Scoring Rules](#3-scoring-rules)
- [4. Evidence Standard](#4-evidence-standard)
- [5. Conversion Assumptions](#5-conversion-assumptions)
- [6. Audit Process](#6-audit-process)
- [7. HippoTask Core Fields Reference](#7-hippotask-core-fields-reference)
- [8. Versioning](#8-versioning)

---

## 1. How to Use This Rubric

Each provider is scored on **8 dimensions** on a 1–5 scale. The **overall score** is the unweighted mean of all 8, rounded to one decimal.

Dimensions fall into two shapes:

- **Sum dimensions** (D1, D2, D6, D8) — each lists 4–5 sub-criteria worth +1. Score = count of met sub-criteria, capped at 5.
- **Band dimensions** (D3, D4, D5, D7) — each defines 5 numbered bands. Score = the highest band whose criterion the evidence satisfies.

Impressionistic language ("excellent", "painful") is commentary, not scoring. The number must come from this rubric.

**For every cell you score, record a vendor-documentation URL as evidence.** See [§4](#4-evidence-standard).

---

## 2. The 8 Dimensions

| # | Dimension | What We Measure | Shape |
|---|-----------|-----------------|:-----:|
| 1 | **API Completeness** | Can the API natively support HippoTask's core task operations and fields? | sum |
| 2 | **API Ergonomics** | How consistent is the protocol, SDK, docs, and content model? | sum |
| 3 | **Schema Flexibility** | How extensible is the task schema via custom fields? | band |
| 4 | **Rate Limits** | How much throughput does a single account sustain? | band |
| 5 | **Real-Time** | Can we receive push notifications for changes? | band |
| 6 | **Auth Simplicity** | How easy is it to authenticate (PAT, OAuth, enterprise)? | sum |
| 7 | **Data Portability** | Can we extract all data without lock-in? | band |
| 8 | **AI/Agent Readiness** | Does the platform expose programmable AI/agent integration (MCP, agent APIs)? | sum |

---

## 3. Scoring Rules

### D1 — API Completeness (sum, cap 5)

- [ ] Full task CRUD (create, read, update, delete) via native endpoints
- [ ] Full container CRUD (projects / plans / boards / lists)
- [ ] ≥ 80% of HippoTask's 14 core fields mappable natively (see [§7](#7-hippotask-core-fields-reference)) — ≥ 12 of 14; not via custom-field workaround
- [ ] Native subtask / parent-child hierarchy (one task can declare another as its parent, with bidirectional traversal)
- [ ] No major operation gap — see definition below

**"Major operation gap"** = any one of: (a) cannot move tasks across containers via API, (b) cannot transition status via API, (c) cannot assign/unassign a user via API, (d) cannot set or update `due_date` on an existing task, (e) the task entity cannot be deleted (soft or hard) via API. Missing bulk operations is **not** a major gap (covered by D4). Missing a single optional field is not a gap (covered by the 80%-native rule).

### D2 — API Ergonomics (sum, cap 5)

- [ ] Single consistent API model (one REST or one GraphQL — not split across paradigms for the same core entity; a REST CRUD + GraphQL-only companion feature like ProjectsV2 *does* count as split)
- [ ] Vendor-maintained TypeScript/JavaScript SDK with a release in the last 180 days
- [ ] Public changelog with entries in the last 180 days
- [ ] Rich-text descriptions use a standard format (Markdown, HTML, or plain text) — not a proprietary tree (e.g., ADF, Notion blocks)
- [ ] Standardized pagination (cursor or page/offset) with documented shape

**"Vendor-maintained SDK"** = published under the vendor's GitHub org or npm/PyPI org (or explicitly endorsed on the vendor's official docs page as first-party), with a release authored by the vendor in the last 180 days. Community forks, unofficial OpenAPI-generated clients, and ecosystem-partner libraries do **not** count, even if high-quality.

### D3 — Schema Flexibility (band)

| Score | Criteria |
|:-:|---|
| 5 | Unlimited or fully dynamic schema with ≥ 10 typed custom-field types |
| 4 | Typed custom fields with practical limit (≥ 50 per container) and ≥ 5 field types |
| 3 | Custom fields, but limited (strict cap, < 5 types, or paywall / tier-gated) |
| 2 | Labels/tags only — no typed custom fields |
| 1 | Fixed schema, no user-configurable extensions |

### D4 — Rate Limits (band — effective operations/minute)

An **"operation"** = one logical read or write of a single task-like entity, measured at the tightest bottleneck of the programmatic-integration auth path (see [§5](#5-conversion-assumptions)). Compute **effective ops/min** per the API paradigm below:

| Paradigm | Formula for ops/min |
|---|---|
| REST, one call per task | documented rpm |
| REST + bulk endpoint (e.g., `/issue/bulk`, `$batch`) | `rpm × max_items_per_bulk` |
| GraphQL, one query per task | documented rpm |
| GraphQL with documented aliasing or multi-mutation support | `rpm × 10` (conservative default, see §5) |
| Sync API with batched commands (e.g., Todoist `/sync`) | `rpm × max_commands_per_sync` |
| Point/complexity-based | `points_per_minute ÷ cost_per_op` where `cost_per_op` = vendor-documented cost of a single task GET, or 1 if undocumented |

If the vendor documents multiple rate-limit ceilings (per-token, per-IP, daily cap, complexity budget), use the **tightest one the programmatic integration actually hits**. Ignore caps that apply only to unrelated workloads (e.g., UI API, webhook delivery).

| Score | Effective ops/min |
|:-:|---|
| 5 | ≥ 1,000 |
| 4 | 500–999 |
| 3 | 150–499 |
| 2 | 50–149 |
| 1 | < 50 |

### D5 — Real-Time (band)

| Score | Criteria |
|:-:|---|
| 5 | Webhooks cover all major entities, ≥ 30 documented event identifiers, delivery retries documented |
| 4 | Webhooks cover main entities, 15–29 event identifiers |
| 3 | Webhooks exist for main entities OR mature delta-query alternative; < 15 event identifiers |
| 2 | Limited webhooks (add-on-only, board-scoped only, or poor delivery / no retries) / delta-polling only |
| 1 | No push mechanism — full polling only |

**Counting event identifiers:** count each distinct identifier the vendor documents as individually subscribable. If the vendor lists `issue_created`, `issue_updated`, `issue_deleted` as three separate webhook topics, that's 3. If a single webhook event (`issues`) carries an `action` field with 10 enum values AND those values are individually filterable/subscribable per the docs, count 10; if they can only all be subscribed together, count 1. When unclear, count the table rows in the vendor's events reference. A "major entity" = task-equivalent, project-equivalent, and user/membership.

### D6 — Auth Simplicity (sum, cap 5)

- [ ] Personal access token (PAT) supported
- [ ] OAuth 2.0 supported
- [ ] PAT generation is self-serve (UI copy-paste, no app registration or review)
- [ ] OAuth public-app registration is self-serve (no vendor-led review / approval gate)
- [ ] Enterprise auth mechanism (service account, app installation token, or SSO-integrated token)

### D7 — Data Portability (band)

| Score | Criteria |
|:-:|---|
| 5 | Full API coverage + dedicated bulk-export endpoint + standard format (JSON/CSV) |
| 4 | Full API coverage + practical API-paged export |
| 3 | Full API coverage; export requires scripting; no dedicated endpoint |
| 2 | API gaps OR proprietary-only export format OR recent (< 180 day) breaking changes raise export cost |
| 1 | No API-based export; data locked in |

**"Dedicated bulk-export endpoint"** = a vendor-documented endpoint whose stated purpose is export, migration, or account-wide snapshot (e.g., Todoist `/sync` full-snapshot, Jira admin CSV export). Bulk-**ingestion** endpoints (`/issue/bulk` create, `$batch`) do **not** count — those are for writes, not extraction. Standard format = any one of JSON, CSV, JSONL, Parquet, or equivalent documented open format.

### D8 — AI/Agent Readiness (sum, cap 5)

- [ ] (+2) Official first-party MCP server — see definition below
- [ ] (+1) Public agent/AI API — a programmable endpoint (REST/GraphQL/MCP tool) that triggers vendor AI capabilities, distinct from the in-product AI UI
- [ ] (+1) Agent-specific entity or lifecycle — vendor-documented primitives like agent-session state machine, agent-as-user assignment, or agent-scoped OAuth actor
- [ ] (+1) Production signals — **at least 2 of**: (a) named customer case study on vendor site published in last 12 months, (b) integration listed in an official agent-host directory (Claude.ai, ChatGPT agents, Cursor marketplace, Copilot partners), (c) a shipped vendor partnership announcement with a major agent host in last 12 months

**"Official first-party MCP server"** = published under the vendor's GitHub/npm/PyPI org, or hosted under the vendor's own domain, and documented in the vendor's developer portal. Third-party MCP servers that integrate with the platform do **not** count even if high-quality.

---

## 4. Evidence Standard

Every sub-criterion marked met or every band claimed must cite a **canonical vendor URL**:

- ✅ Developer portals, API reference, changelog entries, rate-limit pages, auth docs
- ❌ Marketing pages, community blog posts, third-party summaries (fallback only if no vendor doc exists for the specific claim)

Claims without citations are not scored. A pending release ("coming soon", "in preview") is worth **zero** until the vendor ships and documents it — unless the preview is publicly usable with a documented endpoint, in which case it may count with a note.

Cite the URL that *most directly* supports the claim. For a rate-limit number, cite the rate-limit page — not a blog post that mentions the number.

---

## 5. Conversion Assumptions

These defaults apply when the vendor doesn't document a value directly. Document any deviation per-platform.

- **Point-based rate limits** — assume `cost_per_op = 1` point unless the vendor publishes a typical cost for a single-entity GET. Mutations often cost more; annotate per-platform if the distinction moves the score.
- **Integration tier (D4)** — score the API-access tier a programmatic integration would actually use:
  - Start with the **cheapest tier that grants the programmatic-integration auth method** (personal token, app token, OAuth integration token) **and** sustains ≥ 10 ops/min at its documented rate limit.
  - If Free qualifies, score Free.
  - If Free grants API access but its rate limit is below 10 ops/min (meaning Free is not integration-capable), score the cheapest paid tier that *is* integration-capable.
  - If the vendor offers a plan-independent programmatic auth path (e.g., Monday personal tokens at 10M pts/min regardless of plan), use that path — daily call caps that apply only to unrelated workloads (UI API, webhook delivery) are ignored.
- **GraphQL multiplexing** — when a platform doesn't document a bulk ceiling, assume `typical_batch_size = 10` for D4 (conservative aliased multi-operation). If the vendor documents a higher ceiling, use it.
- **"Last 180 days"** — wherever the rubric says "in the last 6 months", this means within 180 days of the audit date. Audits are dated in `docs/scorecard-history.json`.
- **"OAuth 2.0"** — includes OAuth 2.1 and any successor standard the vendor frames as OAuth-compatible. Historical OAuth 1.0a does **not** count.
- **Recent breaking changes** — if a breaking change landed within 180 days of audit, the D2 "pagination / changelog" sub-criterion stays met only if the vendor published a migration guide before the breaking change took effect.
- **"Native" for D1** — a field is native when the vendor's API exposes it directly on the task entity at CRUD time. A field that requires a user-configured custom field, Power-Up, premium add-on, or separate sub-resource counts as **non-native**.
- **Preview/beta features** — a feature in vendor-documented preview or beta **counts** if it has a stable public endpoint and is usable without a waitlist. A feature with "coming soon" / "waitlist only" / undocumented does **not** count.

---

## 6. Audit Process

### Cadence

- **Quarterly** full re-audit of all providers
- **Ad hoc patch audits** when a single provider ships a material API change (e.g., MCP server launch, new auth type, rate-limit change, schema split)
- **Annual** review of this rubric itself — see [§8](#8-versioning)

### Per-Provider Steps

For each provider in scope:

1. **Fetch evidence** — open the current API reference, changelog (last 6 months), rate-limit page, webhook list, auth docs.
2. **Score each dimension** against [§3](#3-scoring-rules) — mark every sub-criterion or band condition, capture the evidence URL.
3. **Compute** — sum (capped at 5) for sum-dimensions; pick highest satisfied band for band-dimensions.
4. **Overall** = mean of 8 dimensions, rounded to 1 decimal.
5. **Diff** against the previous audit — every changed cell gets a one-line justification.
6. **Publish** — update `docs/PROVIDER_SCORECARD.md` §2 (table) and §3 (detailed ratings). Bump the scorecard version (see [§8](#8-versioning)). Update the audit date at the top of §2.
7. **Log** — append `{date, scorecard_version, rubric_version, changes, provider_deltas}` to `docs/scorecard-history.json`.

### Disputing a Cell

A reader who believes a score is wrong should:

1. Open a GitHub Issue citing a vendor-doc URL showing the sub-criterion is met (or not).
2. Maintainers re-apply [§3](#3-scoring-rules) against the new evidence.
3. If the cell changes, publish a **patch audit** — bump the scorecard patch version and log the delta.

---

## 7. HippoTask Core Fields Reference

D1's "≥ 80% native fields" criterion counts these **14** fields. Use this exact list — don't invent variants:

| # | Field | Description |
|---|-------|-------------|
| 1 | `id` | Platform-generated unique identifier |
| 2 | `title` | Task title / name / summary |
| 3 | `description` | Long-form content (Markdown / HTML / plain text / blocks) |
| 4 | `status` | Lifecycle state — must support the closed set `backlog / todo / in_progress / in_review / done / cancelled` |
| 5 | `priority` | Closed set `none / low / medium / high / urgent` |
| 6 | `assignee` | User(s) responsible |
| 7 | `labels` | Multi-label classification |
| 8 | `due_date` | Target completion |
| 9 | `start_date` | Scheduled start |
| 10 | `parent_id` | Subtask parent reference |
| 11 | `estimated_hours` | Effort estimate |
| 12 | `logged_hours` | Actual time spent |
| 13 | `created_at` | Creation timestamp |
| 14 | `updated_at` | Last-modified timestamp |

**80% threshold = ≥ 12 of 14** fields native.

Source of truth for the field list: [`packages/core/src/schema/task.ts`](../packages/core/src/schema/task.ts). If the schema adds or removes a field, bump the rubric **minor** version and update this table.

---

## 8. Versioning

This rubric follows a semver-like convention:

- **Major** — any change that could alter an existing score (adding/removing sub-criteria, changing band thresholds). Triggers a full re-audit of every provider.
- **Minor** — additions that don't change existing scores (new conversion assumption, updated core-fields list when the schema grows, documentation clarification that preserves intent).
- **Patch** — typo / formatting fixes.

Bumping the **major** version invalidates previous audits for direct comparison until all providers are re-scored. Always re-audit the full set when bumping major.

### Rubric Changelog

| Version | Date | Changes |
|---|---|---|
| 2.1.0 | 2026-04-19 | Tightening pass for publication readiness. Specifies per-paradigm `ops/min` formula table in D4; replaces "lowest commonly-integrated tier" with an objective "integration-capable tier" rule in §5; defines "operation", "major operation gap", "vendor-maintained SDK", "dedicated bulk-export endpoint", "official first-party MCP server", and "event identifier"; replaces D8 "production signals" with a ≥ 2-of-3 criterion; normalizes "last 6 months" to "180 days"; clarifies OAuth 2.0 covers 2.1+, excludes 1.0a; documents preview/beta counting rule. Deterministic recomputation (via generator in scorecard 2.0.1) caught two hand-scoring errors in v2.0.0: Monday D1 (3→4) and MS Planner D4 (4→5). |
| 2.0.0 | 2026-04-19 | First deterministic version. Replaces narrative 1–5 descriptors with sum/band sub-criteria. Adds conversion assumptions (§5) and audit process (§6). Introduces the 14 core fields reference (§7). |
| 1.0.0 | 2026-03-08 | Initial rubric — 1–5 narrative descriptors per dimension (not fully deterministic). |
