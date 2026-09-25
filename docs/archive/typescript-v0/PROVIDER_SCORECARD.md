# HippoTask — Provider Scorecard

> How open, interoperable, and developer-friendly is each task management platform?
>
> This scorecard rates every supported provider on objective metrics so users know which platforms play nicely with HippoTask — and which will fight them.

---

## Table of Contents

- [1. Scoring Methodology](#1-scoring-methodology)
- [2. The Scorecard](#2-the-scorecard)
- [3. Detailed Ratings](#3-detailed-ratings)
- [4. Scorecard Visualization](#4-scorecard-visualization)
- [5. What This Means for Users](#5-what-this-means-for-users)
- [6. Maintaining the Scorecard](#6-maintaining-the-scorecard)

---

## 1. Scoring Methodology

Each provider is rated on **8 dimensions** on a 1–5 scale. Scores are derived **deterministically** from [**SCORECARD_RUBRIC.md**](SCORECARD_RUBRIC.md) — given the same evidence, two auditors produce the same result. The overall score is the unweighted mean of all 8 dimensions, rounded to one decimal.

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

**Sum dimensions** score +1 per sub-criterion met (capped at 5). **Band dimensions** pick the highest band whose condition the evidence satisfies. For the full sub-criterion list, band thresholds, evidence standard, and conversion assumptions, see [SCORECARD_RUBRIC.md](SCORECARD_RUBRIC.md). The rubric is the authoritative source; this scorecard is its output.

Impressionistic language ("excellent", "painful") in §3 is commentary, not scoring; the number comes from the rubric.

---

## 2. The Scorecard

<!-- BEGIN:AUTOGEN_VERIFIED_AT -->
*Verified: **2026-04-19** — rubric v2.1.0, scorecard v2.0.1*
<!-- END:AUTOGEN_VERIFIED_AT -->

> The table below is generated from [`docs/scorecard/providers/*.yaml`](scorecard/providers). Run `pnpm scorecard:generate` after editing any provider YAML. CI enforces no drift via `pnpm scorecard:validate`.

<!-- BEGIN:AUTOGEN_SCORECARD_TABLE -->
| Provider | API Complete | API Ergo | Schema Flex | Rate Limits | Real-Time | Auth Simple | Data Port | AI Ready | **Overall** |
|----------|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| **Jira** | 5 | 3 | 5 | 5 | 5 | 4 | 4 | 5 | **4.5** |
| **GitHub** | 4 | 4 | 3 | 4 | 5 | 5 | 5 | 5 | **4.4** |
| **Linear** | 4 | 5 | 2 | 4 | 5 | 5 | 4 | 5 | **4.3** |
| **Monday.com** | 4 | 5 | 5 | 5 | 4 | 4 | 3 | 4 | **4.3** |
| **Asana** | 4 | 5 | 4 | 3 | 4 | 5 | 4 | 3 | **4.0** |
| **ClickUp** | 5 | 4 | 5 | 2 | 4 | 4 | 3 | 4 | **3.9** |
| **Todoist** | 4 | 5 | 2 | 5 | 4 | 4 | 4 | 3 | **3.9** |
| **Notion** | 3 | 4 | 5 | 3 | 3 | 5 | 2 | 4 | **3.6** |
| **Trello** | 2 | 4 | 3 | 4 | 5 | 3 | 4 | 1 | **3.3** |
| **MS Planner** | 2 | 5 | 2 | 5 | 2 | 2 | 2 | 4 | **3.0** |
<!-- END:AUTOGEN_SCORECARD_TABLE -->

### Notable changes since the March 2026 audit (v1.0)

- **Notion +1.2** (2.4 → 3.6) — [webhooks shipped](https://developers.notion.com/reference/webhooks) and [official MCP server](https://developers.notion.com/docs/mcp) + [Notion 3.0 Agents](https://www.notion.com/releases/2025-09-18) cleared its two historical weaknesses.
- **Monday +1.2** (3.1 → 4.3) — [first-party MCP server](https://github.com/mondaycom/mcp) + GraphQL multiplexing credited on D4. v2.0.1 recount corrected a D1 hand-scoring error (+0.2).
- **Todoist +0.8** (3.1 → 3.9) — [unified API v1](https://developer.todoist.com/api/v1/) + `move_task` fixed historical gotcha; [official MCP server](https://github.com/Doist/todoist-ai) + Sync batching on D4.
- **MS Planner +0.7** (2.3 → 3.0) — Copilot Studio MCP support + Project Manager agent. v2.0.1 recount corrected a D4 hand-scoring error (+0.1).
- **Jira +0.4** (4.1 → 4.5) — [Atlassian Remote MCP](https://www.atlassian.com/platform/remote-mcp-server) GA + [Rovo agents](https://developer.agent.atlassian.com/) as assignees flipped its weakest dimension into its strongest.
- **Linear −0.2** (4.5 → 4.3) — reranked because deterministic rate-limit scoring no longer gives full credit for undocumented "generous in practice"; raw 5,000 req/hr with complexity budget places it in band 4.

---

## 3. Detailed Ratings

Each subsection below is generated from [`docs/scorecard/providers/*.yaml`](scorecard/providers). Cell-by-cell vendor-documentation citations live in those files.

---

<!-- BEGIN:AUTOGEN_DETAILED_RATINGS -->
### 🥇 Jira — Overall: 4.5 / 5

**D1 — API Completeness: 5/5**
- ✓ Full task CRUD via [Issues API](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/)
- ✓ Full project CRUD via [Projects API](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-projects/)
- ✓ 13/14 native fields (`start_date` via `customfield_10015`, not guaranteed)
- ✓ Native subtasks (`parent.key` + subtask issuetype)
- ✓ No major ops gaps — [bulk create/edit/delete/transition](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/)

**D2 — API Ergonomics: 3/5**
- ✓ Single REST v3 model — [v3 intro](https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/) (v2 deprecated)
- ✗ No first-party JS/TS SDK — [`jira.js`](https://github.com/MrRefactoring/jira.js) is community
- ✓ [Changelog](https://developer.atlassian.com/cloud/jira/platform/changelog/) active
- ✗ ADF (Atlassian Document Format) for rich text — proprietary JSON tree; [JRACLOUD-77436](https://jira.atlassian.com/browse/JRACLOUD-77436) (Markdown API) still unresolved
- ✓ Cursor-paged `/search/jql` — replaced deprecated `/search` on 2025-10-31

**D3 — Schema Flexibility: 5/5**
Unlimited typed custom fields with dynamic schema — [Fields API](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-fields/). Soft guidance ≤ 1,000 fields/instance for performance; no hard cap.

**D4 — Rate Limits: 5/5**
Tier scored: site-wide (all plans). 65,000 points/hour ÷ 1 pt/GET × 1/60 min ≈ 1,083 ops/min. [65,000 points/hour per site](https://developer.atlassian.com/cloud/jira/platform/rate-limiting/). Bulk endpoints push effective throughput higher (50 issues per bulk-create, 1,000 IDs per bulk-edit).

**D5 — Real-Time: 5/5**
30 documented event identifiers. 30+ [webhook event identifiers](https://developer.atlassian.com/cloud/jira/platform/webhooks/) covering issues, comments, worklogs, sprints, projects, users, versions, attachments. Retry semantics documented.

**D6 — Auth Simplicity: 4/5**
- ✓ [API token](https://id.atlassian.com/manage-profile/security/api-tokens) (basic auth)
- ✓ [OAuth 2.0 3LO](https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/)
- ✓ Token self-serve from Atlassian profile
- ✗ OAuth app registration requires Atlassian developer console + scope config
- ✓ [Connect](https://developer.atlassian.com/cloud/jira/platform/getting-started-with-connect/) / [Forge](https://developer.atlassian.com/platform/forge/) for enterprise

**D7 — Data Portability: 4/5**
Full API coverage + bulk endpoints. No single "export everything" endpoint; admin CSV/JSON export exists via console but is not API-first.

**D8 — AI/Agent Readiness: 5/5**
- ✓ (+2) [Atlassian Remote MCP Server](https://www.atlassian.com/platform/remote-mcp-server) — GA 2025, Cloudflare-hosted, OAuth 2.1
- ✓ (+1) Rovo Agents public API — Forge [`rovo-agent` module](https://developer.atlassian.com/platform/forge/manifest-reference/modules/rovo-agent/)
- ✓ (+1) Rovo agents assignable as Jira assignees (beta Feb 2026)
- ✓ (+1) Atlassian-led Claude / Rovo integration + published Rovo case studies

**Bottom line:** Jira leads overall — 2025's MCP + Rovo rollout flipped its historically weakest dimension into its strongest. ADF and no first-party SDK remain real papercuts.

---

### 🥈 GitHub — Overall: 4.4 / 5

**D1 — API Completeness: 4/5**
- ✓ Full issue CRUD — [REST Issues](https://docs.github.com/en/rest/issues/issues)
- ✓ Full ProjectsV2 CRUD — [GraphQL](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects)
- ✗ 8/14 native (priority, due_date, start_date, estimated_hours, logged_hours all require ProjectsV2 custom fields; issue `state` is binary not full status)
- ✓ Native sub-issues REST API — [GA Dec 2024](https://github.blog/changelog/2024-12-12-github-issues-projects-close-issue-as-a-duplicate-rest-api-for-sub-issues-and-more/)
- ✓ No ops gaps — issues moveable across projects

**D2 — API Ergonomics: 4/5**
- ✗ Dual model — REST for Issues, GraphQL-only for ProjectsV2
- ✓ [Octokit](https://github.com/octokit) — first-party JS/TS, Ruby, Go, .NET (no official Python; PyGithub is community)
- ✓ [Changelog](https://github.blog/changelog/) + [breaking changes](https://docs.github.com/en/rest/about-the-rest-api/breaking-changes)
- ✓ Markdown bodies throughout
- ✓ Standard cursor pagination (GraphQL) / Link-header pagination (REST)

**D3 — Schema Flexibility: 3/5**
ProjectsV2 supports 5 typed field types (text, number, date, single-select, iteration) with 50-option cap on single-select; no `updateProjectV2Field` mutation (option edits are delete+recreate = data loss). Custom fields are project-scoped, not issue-scoped. [Understanding fields](https://docs.github.com/en/issues/planning-and-tracking-with-projects/understanding-fields)

**D4 — Rate Limits: 4/5**
Tier scored: PAT authenticated. REST 5,000/hr = 83 rpm; GraphQL 5,000 pts/hr × 10 aliases = 833 ops/min. [REST PAT: 5,000/hr](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api); [GraphQL: 5,000 points/hr](https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api). With GraphQL aliased multi-mutation, effective throughput ≈ 830 ops/min (83 req/min × 10 typical aliases). Enterprise Cloud raises to 10,000–15,000/hr.

**D5 — Real-Time: 5/5**
60 documented event identifiers. ~60 [webhook event types](https://docs.github.com/en/webhooks/webhook-events-and-payloads) including `issues`, `issue_comment`, `projects_v2*`, `sub_issues`. Documented retry semantics.

**D6 — Auth Simplicity: 5/5**
- ✓ Classic PAT + [fine-grained PAT](https://github.blog/changelog/2025-03-18-fine-grained-pats-are-now-generally-available/) (GA 2025-03-18)
- ✓ [OAuth App](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps) + [GitHub App](https://docs.github.com/en/apps/creating-github-apps)
- ✓ Self-serve PAT
- ✓ Self-serve OAuth / App registration
- ✓ GitHub App installations for enterprise

**D7 — Data Portability: 5/5**
Full API + Git-native storage (inherently portable) + documented [migration tooling](https://docs.github.com/en/migrations).

**D8 — AI/Agent Readiness: 5/5**
- ✓ (+2) [github/github-mcp-server](https://github.com/github/github-mcp-server) — v1.0.0 GA 2026-04-16, issues + projects + repo surface
- ✓ (+1) Copilot coding agent [GA 2025-09-25](https://github.blog/changelog/2025-09-25-copilot-coding-agent-is-now-generally-available/) — assignable to issues
- ✓ (+1) Copilot-as-assignee (agent-as-user pattern)
- ✓ (+1) [Copilot CLI GA](https://www.infoq.com/news/2026/04/github-copilot-cli-ga/) March 2026; broad agent-host partnerships

**Bottom line:** GitHub holds 4.4. Strong everywhere except schema flex, with the dual Issues/ProjectsV2 model as its enduring ergonomic cost.

---

### 🥉 Linear — Overall: 4.3 / 5

**D1 — API Completeness: 4/5**
- ✓ Full issue CRUD — [GraphQL](https://linear.app/developers/graphql)
- ✓ Full project CRUD
- ✗ 11/14 native (78.5% — just under 80%; `start_date` is read-only `startedAt`, `estimated_hours` is unitless points, `logged_hours` missing)
- ✓ Native sub-issues (`Issue.parent`)
- ✓ No ops gaps

**D2 — API Ergonomics: 5/5**
- ✓ Single GraphQL model
- ✓ [`@linear/sdk`](https://www.npmjs.com/package/@linear/sdk) v80.1.0 — first-party TS, active
- ✓ [Changelog](https://linear.app/changelog) with `[API]` prefix for API entries
- ✓ Markdown descriptions
- ✓ Cursor pagination

**D3 — Schema Flexibility: 2/5**
No arbitrary user-defined custom fields. Extensibility = labels, projects, cycles, and (June 2025) [Asks fields](https://linear.app/changelog/2025-06-05-asks-fields-and-triage-routing) scoped to triage — not general-purpose.

**D4 — Rate Limits: 4/5**
Tier scored: API key. 5,000 req/hr × 10 aliases ÷ 60 min ≈ 833 ops/min. [5,000 req/hr + 3M complexity pts/hr](https://linear.app/developers/rate-limiting) on API keys; 10,000 pts max per query. At 10-alias typical batch → effective ~830 ops/min. Complexity budget lets denser queries fetch hundreds of items per request.

**D5 — Real-Time: 5/5**
40 documented event identifiers. 14 entity types × 3 action types + specialized (Issue SLA, OAuthApp revoked, **Agent session events**) = 40+ [webhook events](https://linear.app/developers/webhooks).

**D6 — Auth Simplicity: 5/5**
- ✓ [Personal API keys](https://linear.app/developers/oauth-2-0-authentication)
- ✓ [OAuth 2.0](https://linear.app/developers/oauth-2-0-authentication)
- ✓ PAT self-serve
- ✓ OAuth app registration self-serve
- ✓ [OAuth actor authorization](https://linear.app/developers/oauth-actor-authorization) for agents/apps as actors

**D7 — Data Portability: 4/5**
Full GraphQL coverage — everything is fetchable via paged queries. No dedicated bulk-export endpoint.

**D8 — AI/Agent Readiness: 5/5**
- ✓ (+2) [Linear MCP server](https://linear.app/docs/mcp) — hosted, find/create/update for issues/projects/comments
- ✓ (+1) [Agent Interaction SDK](https://linear.app/now/our-approach-to-building-the-agent-interaction-sdk) (Developer Preview)
- ✓ (+1) [Agent Sessions API](https://linear.app/developers/agents) — agents as first-class users with session lifecycle
- ✓ (+1) Published case studies (Rivet May 2025, Reflag) + webhook event category for agents

**Bottom line:** Linear remains the DX leader and has the most mature agent primitives (sessions, actor-based auth). Lack of custom fields is the only real weakness.

---

### Monday.com — Overall: 4.3 / 5

**D1 — API Completeness: 4/5**
- ✓ Full item CRUD
- ✓ Full board/group CRUD
- ✗ 5/14 native (id, title, parent_id, created_at, updated_at); all other fields require column-type custom fields
- ✓ Native subitems (`create_subitem`)
- ✓ No ops gaps

**D2 — API Ergonomics: 5/5**
- ✓ Single GraphQL model
- ✓ [`monday-sdk-js`](https://www.npmjs.com/package/monday-sdk-js) — first-party TS/JS
- ✓ [Changelog](https://developer.monday.com/api-reference/changelog) active
- ✓ Text / long-text column types in standard formats
- ✓ Cursor pagination

**D3 — Schema Flexibility: 5/5**
Fully dynamic schema — 27+ [column types](https://developer.monday.com/api-reference/reference/column-types-reference) including status, dropdown, text, long_text, numbers, date, timeline, people, tags, checkbox, link, email, phone, rating, color_picker, dependency, file, formula, mirror, connect_boards, subitems, time_tracking, hour, location, vote, world_clock, country. No hard cap on columns per board.

**D4 — Rate Limits: 5/5**
Tier scored: Free (personal token, plan-independent programmatic path). Personal token 1M pts/min on Free ÷ 1 pt/op (GET) = 1,000,000 ops/min. Personal token [10M complexity pts/min](https://developer.monday.com/api-reference/docs/rate-limits) (1M Free/Trial). `create_item` ≈ 10k pts → ≈ 1,000 writes/min; reads far more. Daily call caps apply at low tiers but complexity is the binding constraint for integrations.

**D5 — Real-Time: 4/5**
15 documented event identifiers. ~15+ [webhook events](https://developer.monday.com/api-reference/reference/webhooks) including `change_column_value`, `change_status_column_value`, `create_item`, `create_subitem`, `item_archived/deleted/moved`, `create_update`, plus 12 app-lifecycle events. Board-scoped registration.

**D6 — Auth Simplicity: 4/5**
- ✓ V2 personal API tokens
- ✓ [OAuth 2.0](https://developer.monday.com/apps/docs/oauth)
- ✓ PAT self-serve
- ✓ OAuth app self-serve
- ✗ No service-account / SSO-integrated token for enterprise (OAuth-only path)

**D7 — Data Portability: 3/5**
Full GraphQL API. No dedicated bulk export; board-based model requires per-board schema introspection before extraction.

**D8 — AI/Agent Readiness: 4/5**
- ✓ (+2) [`@mondaydotcomorg/monday-api-mcp`](https://github.com/mondaycom/mcp) — first-party, ~15 tools + dynamic GraphQL beta
- ✓ (+1) [Sidekick](https://monday.com/w/mcp), Vibe, AI Blocks — programmable AI surface
- ✗ No agent-specific entity/lifecycle documented
- ✓ (+1) Hosted MCP integrations with Claude / Cursor / Copilot

**Bottom line:** Dynamic column schema that penalized completeness is precisely what makes flexibility and rate limits excellent. First-party MCP closes the AI gap.

---

### Asana — Overall: 4.0 / 5

**D1 — API Completeness: 4/5**
- ✓ Full task CRUD
- ✓ Full project CRUD
- ✗ 11/14 native (status requires custom field; native priority exists but limited to 3 fixed values)
- ✓ Native subtasks (`parent`)
- ✓ Batch API exists but per-request limits apply

**D2 — API Ergonomics: 5/5**
- ✓ Single REST model
- ✓ [`asana` on npm](https://www.npmjs.com/package/asana) + [PyPI](https://pypi.org/project/asana/) — first-party, active (Node/Python); Java/PHP/Ruby deprecated
- ✓ [Changelog](https://forum.asana.com/c/forum-en/api/api-changelog/204) (forum category — public, active)
- ✓ HTML / plain text notes
- ✓ `opt_fields` + cursor pagination

**D3 — Schema Flexibility: 4/5**
Typed custom fields: text, number, date, single-select, multi-select, people, formula (read-only). [100 fields/project](https://developers.asana.com/docs/custom-fields-guide) (raised from 30). Workspace-scoped.

**D4 — Rate Limits: 3/5**
Tier scored: Free. 150 rpm on Free tier (Paid tier would score band 5 at 1,500 rpm). [Free: 150 req/min; Paid: 1,500 req/min](https://developers.asana.com/docs/rate-limits). Score at Free = band 3 (Paid would be band 5). Search capped at 60/min. Duplication jobs capped at 5 concurrent.

**D5 — Real-Time: 4/5**
15 documented event identifiers. Resource-level [webhooks](https://developers.asana.com/docs/webhooks-guide) with HMAC signing, exponential-backoff retry, 24h failure expiry. Compact events (re-fetch required). ~15+ event combinations.

**D6 — Auth Simplicity: 5/5**
- ✓ [PAT](https://developers.asana.com/docs/personal-access-token)
- ✓ [OAuth 2.0](https://developers.asana.com/docs/oauth)
- ✓ PAT self-serve
- ✓ OAuth self-serve
- ✓ Service Accounts (Enterprise) — see [Authentication](https://developers.asana.com/docs/authentication)

**D7 — Data Portability: 4/5**
Full API + batch + CSV export (UI). Tasks-in-multiple-projects adds extraction complexity but is tractable.

**D8 — AI/Agent Readiness: 3/5**
- ✓ (+2) [Asana MCP Server V2](https://developers.asana.com/docs/mcp-server) — GA Feb 2026, streamable HTTP, ~42 tools (V1 SSE server sunset 2026-05-11)
- ✗ AI Studio / Asana Intelligence has no public API
- ✗ No agent-specific entity
- ✓ (+1) Claude integration (conversations → projects)

**Bottom line:** Asana's MCP V2 + Service Accounts raise the floor considerably. AI Studio being product-only rather than programmable is the remaining gap.

---

### ClickUp — Overall: 3.9 / 5

**D1 — API Completeness: 5/5**
- ✓ Full task CRUD
- ✓ Full list/folder/space CRUD
- ✓ 14/14 native (rare — includes native time-tracking read/write)
- ✓ Native subtasks
- ✓ Tasks moveable across lists

**D2 — API Ergonomics: 4/5**
- ✓ REST v2 stable, v3 endpoints [shipping gradually](https://developer.clickup.com/) (dual version adds minor confusion)
- ✗ No first-party SDK (community only: `clickup-python-sdk`, `pyclickup`)
- ✓ [Changelog](https://feedback.clickup.com/changelog/api-v2)
- ✓ `markdown_description` supported
- ✓ Standard pagination

**D3 — Schema Flexibility: 5/5**
~18 [custom field types](https://developer.clickup.com/docs/customfields) (text, number, currency, date, checkbox, email, phone, URL, dropdown, labels, rating, progress, people, tasks, location, emoji, formula, files). 500 options per dropdown/label. Free tier capped at 60 total field uses; paid unlimited.

**D4 — Rate Limits: 2/5**
Tier scored: Free / Unlimited / Business. 100 rpm at entry tiers (Business Plus 1,000 rpm would score band 5; Enterprise 10,000 rpm). [Per-token, per-minute](https://developer.clickup.com/docs/rate-limits): Free/Unlimited/Business = **100 rpm**, Business Plus = 1,000, Enterprise = 10,000. Score at Free = band 2. Business Plus would be band 5.

**D5 — Real-Time: 4/5**
20 documented event identifiers. Comprehensive [webhooks](https://developer.clickup.com/docs/webhooks) — task / list / folder / space / goal / keyResult × create/update/delete plus `*` wildcard. ~20+ events.

**D6 — Auth Simplicity: 4/5**
- ✓ Personal token (`pk_` prefix, never expires)
- ✓ [OAuth 2.0](https://developer.clickup.com/docs/authentication)
- ✓ PAT self-serve
- ✓ OAuth self-serve
- ✗ No dedicated service-account / enterprise-token tier

**D7 — Data Portability: 3/5**
Full API coverage. No bulk-export endpoint. 5-level hierarchy (Workspace → Space → Folder → List → Task → Subtask) means extraction requires walking the tree.

**D8 — AI/Agent Readiness: 4/5**
- ✓ (+2) [Official ClickUp MCP server](https://developer.clickup.com/docs/connect-an-ai-assistant-to-clickups-mcp-server) (OAuth-only, task CRUD + Brain context)
- ✓ (+1) [ClickUp Super Agents](https://clickup.com/blog/mcp-tools/) GA Dec 22, 2025
- ✗ No agent-specific entity pattern
- ✓ (+1) 2026 roadmap for external MCP tool integration

**Bottom line:** Completeness leader (14/14 native) with MCP and Super Agents closing the AI gap. Free-tier rate limit and no first-party SDK are the drag.

---

### Todoist — Overall: 3.9 / 5

**D1 — API Completeness: 4/5**
- ✓ Full task CRUD — [unified v1](https://developer.todoist.com/api/v1/)
- ✓ Full project CRUD
- ✗ 9/14 native (no status beyond `is_completed`/`checked`; no `start_date`; no `logged_hours`; `updated_at` not consistently exposed)
- ✓ Native subtasks (`parent_id`)
- ✓ `move_task` command — historical "can't move between projects" gotcha is fixed

**D2 — API Ergonomics: 5/5**
- ✓ Single unified v1 model (REST v2 + Sync v9 deprecating early 2026)
- ✓ [`@doist/todoist-api-typescript`](https://github.com/Doist/todoist-api-typescript) v9.3 (2026-04-18), [`todoist-api-python`](https://github.com/Doist/todoist-api-python) v4.x — both first-party, active
- ✓ [Changelog](https://groups.google.com/a/doist.com/g/todoist-api) (Google Group)
- ✓ Markdown descriptions
- ✓ Cursor pagination

**D3 — Schema Flexibility: 2/5**
Labels only — no user-defined custom fields.

**D4 — Rate Limits: 5/5**
Tier scored: per-user (all plans). 1,000 req / 15 min ≈ 67 rpm × 100 commands/sync = 6,700 ops/min. [1,000 req / 15 min REST](https://developer.todoist.com/api/v1/) ≈ 67 rpm, but `/sync` batches up to **100 commands per request** → effective 6,700 ops/min — band 5.

**D5 — Real-Time: 4/5**
15 documented event identifiers. [Webhooks](https://developer.todoist.com/api/v1/#tag/Webhooks) with HMAC-SHA256 signature. Events: `item:added|updated|deleted|completed|uncompleted`, `note:*`, `project:*`, `label:*`, `filter:*`, `reminder:fired`. ~15+ events.

**D6 — Auth Simplicity: 4/5**
- ✓ Personal API token
- ✓ [OAuth 2.0](https://developer.todoist.com/api/v1/#tag/Authentication)
- ✓ Token self-serve
- ✓ OAuth self-serve
- ✗ No dedicated enterprise auth tier

**D7 — Data Portability: 4/5**
Full API + `/sync` full-snapshot endpoint (100 full-sync req / 15 min).

**D8 — AI/Agent Readiness: 3/5**
- ✓ (+2) [Doist MCP server](https://github.com/Doist/todoist-ai) — hosted at `https://ai.todoist.net/mcp`, streamable HTTP, OAuth
- ✗ No separate agent API
- ✗ No agent entity
- ✓ (+1) Official MCP adoption + ecosystem directory listings

**Bottom line:** Simple but well-rounded — Sync API batching rewards batch-capable clients, unified v1 fixed the move-task gotcha. Labels-only schema is the ceiling.

---

### Notion — Overall: 3.6 / 5

**D1 — API Completeness: 3/5**
- ✓ Full page / database CRUD
- ✓ Full workspace / container CRUD
- ✗ 11/14 native (priority, estimated_hours, logged_hours via custom properties)
- ✓ Native sub-pages via `parent`
- ✗ Description as blocks is a real ops gap — no atomic "set description"

**D2 — API Ergonomics: 4/5**
- ✓ Single REST model
- ✓ [`@notionhq/client`](https://www.npmjs.com/package/@notionhq/client) v5.18+ — first-party TS/JS, active
- ✓ [Changelog](https://developers.notion.com/page/changelog)
- ✗ Rich-text is Notion blocks (proprietary recursive tree) — no standard Markdown surface
- ✓ Standard `start_cursor` / `next_cursor` pagination

**D3 — Schema Flexibility: 5/5**
20+ [property types](https://developers.notion.com/reference/update-property-schema-object) — title, rich_text, number, select, multi_select, status, date, people, files, checkbox, url, email, phone, formula, relation, rollup, created_time, created_by, last_edited_time, last_edited_by, unique_id. No hard count cap.

**D4 — Rate Limits: 3/5**
Tier scored: per-integration (all plans). 3 req/s × 60 s = 180 rpm. [~3 req/s per integration](https://developers.notion.com/reference/request-limits) (token bucket, bursts to 10) — ~180 rpm. No paid tier raises this.

**D5 — Real-Time: 3/5**
10 documented event identifiers. [Webhooks](https://developers.notion.com/reference/webhooks) shipped (2025+) — lightweight event notifications (re-fetch required). No user / workspace-membership events. Event coverage narrower than Jira / GitHub — band 3.

**D6 — Auth Simplicity: 5/5**
- ✓ Internal integration tokens (static bearer, single workspace)
- ✓ [OAuth 2.0](https://developers.notion.com/docs/authorization) public integrations
- ✓ Internal token self-serve
- ✓ OAuth self-serve
- ✓ Enterprise workspace-level OAuth

**D7 — Data Portability: 2/5**
API coverage is complete but [2025-09-03 "Data Source" split](https://developers.notion.com/docs/upgrade-guide-2025-09-03) was breaking; blocks-based export requires recursive fetching against 3 req/s rate; no dedicated bulk export.

**D8 — AI/Agent Readiness: 4/5**
- ✓ (+2) [notion-mcp-server v2](https://github.com/makenotion/notion-mcp-server) — first-party, hosted + self-host
- ✓ (+1) Notion AI readable via normal endpoints
- ✓ (+1) [Notion 3.0 Agents](https://www.notion.com/releases/2025-09-18) — integration-first agent surface (Sep 18, 2025)
- ✗ No published major-host partnership cited at time of audit

**Bottom line:** Notion's 2025 additions (webhooks + MCP + Agents) eliminated its catastrophic ratings. Block-based descriptions and rate limits remain the shape of its ceiling.

---

### Trello — Overall: 3.3 / 5

**D1 — API Completeness: 2/5**
- ✓ Basic card CRUD
- ✓ Full board / list CRUD
- ✗ 9/14 native — no priority, no estimated/logged_hours, `created_at` implicit in ObjectId only
- ✗ No native subtasks — checklists only (checkItems are not cards; `convertToCardFromCheckItem` breaks parent link)
- ✗ Ops gap: status = list position (fragile when users reorder lists)

**D2 — API Ergonomics: 4/5**
- ✓ Single REST `/1/` model (frozen for a decade)
- ✗ No first-party SDK; community [`trello.js`](https://www.npmjs.com/package/trello.js) most actively maintained
- ✓ [Changelog](https://developer.atlassian.com/cloud/trello/changelog/)
- ✓ Markdown `desc`
- ✓ Standard pagination (`before` / `since` / `limit`)

**D3 — Schema Flexibility: 3/5**
Native [custom fields](https://developer.atlassian.com/cloud/trello/rest/api-group-customfields/) — text, number, date, checkbox, list (dropdown). 5 types puts it in band 3.

**D4 — Rate Limits: 4/5**
Tier scored: per-token. 100 req / 10 s per token × 60 s = 600 rpm. [300 req / 10s per API key, 100 req / 10s per token](https://developer.atlassian.com/cloud/trello/guides/rest-api/rate-limits/) — binding = 600 rpm/token. > 200 429s per key → key-wide penalty.

**D5 — Real-Time: 5/5**
30 documented event identifiers. Model-scoped [webhooks](https://developer.atlassian.com/cloud/trello/guides/rest-api/webhooks/) — rich event list (createCard, updateCard, commentCard, addMemberToCard, updateCheckItem, createList, updateList, copyCard, voteOnCard, etc.) ≥ 30.

**D6 — Auth Simplicity: 3/5**
- ✓ API Key + Token
- ✗ OAuth 1.0a only — **no OAuth 2.0** on public REST
- ✓ Token self-serve
- ✓ OAuth self-serve (dated 1.0a flow)
- ✗ No dedicated enterprise auth

**D7 — Data Portability: 4/5**
Full API + per-board JSON export (UI + API). Simple model = easy extraction.

**D8 — AI/Agent Readiness: 1/5**
- ✗ No Trello-specific MCP server (Atlassian Rovo / Remote MCP covers Jira/Confluence, not Trello)
- ✗ No programmable AI API
- ✗ No agent entity
- ✗ No published partnerships for Trello specifically

**Bottom line:** Trello is Atlassian's other task product — but nearly none of the Atlassian 2025/2026 AI investment reached it. Simple, stable, real-time solid; schema and AI are weak.

---

### MS Planner — Overall: 3.0 / 5

**D1 — API Completeness: 2/5**
- ✓ Full task CRUD — [Planner tasks](https://learn.microsoft.com/en-us/graph/api/resources/plannertask?view=graph-rest-1.0)
- ✓ Full plan CRUD
- ✗ 9/14 native (no estimated/logged_hours; `percentComplete` limited to 0/50/100 — status is 3-state not 6-state)
- ✗ No true subtasks (buckets are flat containers; checklist items don't nest)
- ✗ Ops gap: premium / Project features API-inaccessible

**D2 — API Ergonomics: 5/5**
- ✓ Single Graph REST model (v1.0 stable + beta)
- ✓ [Microsoft Graph SDKs](https://learn.microsoft.com/en-us/graph/sdks/sdks-overview) — first-party .NET, JS/TS, Java, Python, Go, PHP, PowerShell
- ✓ [What's new](https://learn.microsoft.com/en-us/graph/whats-new-overview)
- ✓ HTML (beta) / plain-text descriptions
- ✓ `@odata.nextLink` pagination

**D3 — Schema Flexibility: 2/5**
6 boolean categories (v1.0) / 25 (beta) on `plannerAppliedCategories` — plan-level label descriptions, no typed custom fields.

**D4 — Rate Limits: 5/5**
Tier scored: per-app across tenants. 130,000 req / 10 s × 60 s = 780,000 ops/min (per-tenant cap halved 2025-09-30). [130,000 req / 10s per app across tenants](https://learn.microsoft.com/en-us/graph/throttling-limits) globally — generous in aggregate; [per-app/per-user-per-tenant cap was halved 2025-09-30](https://learn.microsoft.com/en-us/graph/throttling). [$batch](https://learn.microsoft.com/en-us/graph/json-batching) allows 20 sub-requests.

**D5 — Real-Time: 2/5**
0 documented event identifiers. Graph [change notifications](https://learn.microsoft.com/en-us/graph/change-notifications-overview) support webhooks + Event Hubs + Event Grid — but **Planner resources are not in the supported-resources list**. Only [delta queries](https://learn.microsoft.com/en-us/graph/delta-query-overview) apply — band 2.

**D6 — Auth Simplicity: 2/5**
- ✗ No PAT — delegated-only
- ✓ Entra ID OAuth 2.0
- ✗ No self-serve token (Azure app registration required)
- ✗ App-only permissions unsupported for Planner
- ✓ Enterprise auth (Entra ID)

**D7 — Data Portability: 2/5**
API coverage limited to basic plans — premium / Project plans inaccessible via Graph. Data locked in M365 ecosystem. No standalone export endpoint.

**D8 — AI/Agent Readiness: 4/5**
- ✓ (+2) Microsoft 365 Copilot Studio [supports MCP](https://devblogs.microsoft.com/microsoft365dev/build-declarative-agents-for-microsoft-365-copilot-with-mcp/) (GA 2025)
- ✓ (+1) Copilot Studio programmable agents
- ✗ Project Manager agent has no public developer API
- ✓ (+1) M365 Copilot ecosystem partnerships

**Bottom line:** Planner is a narrow surface inside Graph. 2025 unification brought premium plans into the UI but not the API. Copilot Studio MCP support is the bright spot.
<!-- END:AUTOGEN_DETAILED_RATINGS -->

---

## 4. Scorecard Visualization

Design for interactive charts on the documentation site and README badges.

### Radar Chart (Per Provider)

Each provider gets a radar/spider chart showing its 8-dimension profile. Example (Notion — post-2025 upgrade):

```
                API Completeness
                      5
                     /|\
                    / | \
        AI Ready 4/  |  \4 API Ergonomics
                /    |    \
               /     |     \
    Data Port 2──────5──────5 Schema Flex
               \     |     /
                \    |    /
      Auth Simple\ 5 |  /3 Rate Limits
                  \  | /
                   \ |/
                    3
               Real-Time
```

### Bar Chart (Comparative)

Side-by-side bars sorted by overall, v2.0:

```
Overall Score (out of 5.0)

Jira         ████████████████████████████████████████████████ 4.5
GitHub       ███████████████████████████████████████████████  4.4
Linear       ██████████████████████████████████████████████   4.3
Monday.com   ████████████████████████████████████████████     4.1
Asana        ███████████████████████████████████████████      4.0
ClickUp      ██████████████████████████████████████████       3.9
Todoist      ██████████████████████████████████████████       3.9
Notion       ██████████████████████████████████████           3.6
Trello       █████████████████████████████████                3.3
MS Planner   ████████████████████████████                     2.9
```

### Implementation Plan

- **Static** (Milestone 1): Markdown table in this document + README badge per provider
- **Interactive** (Milestone 5): Chart.js or D3 radar charts on the documentation site
- **Machine-readable**: `docs/scorecard.json` (planned — matches the structure of §2 + §3 evidence URLs)
- **Badges**: README badges like `![Jira: 4.5/5](https://img.shields.io/badge/Jira-4.5%2F5-brightgreen)`

---

## 5. What This Means for Users

### "Which platforms should I prioritize?"

| If your goal is… | Best platforms | Avoid |
|------------------|----------------|-------|
| Reliable bidirectional sync | Jira, GitHub, Linear | MS Planner (no Planner webhooks), ClickUp Free (100 rpm) |
| Quick read-only aggregation | Any of the top 6 | MS Planner (auth complexity) |
| AI-agent integration | Jira (Rovo + MCP), GitHub (Copilot + MCP), Linear (Agent Sessions) | Trello (no Trello MCP), MS Planner (agent API gated) |
| Maximum field coverage | Jira, ClickUp, Monday.com | Todoist, Linear (labels-only extension) |
| Simple setup | Todoist (auth), Linear (DX), Asana (service accounts) | Jira (ADF), Monday.com (board introspection), MS Planner (Azure AD) |

### "What are the current gotchas?" (v2.0)

<!-- BEGIN:AUTOGEN_GOTCHAS -->
| Provider | Biggest Gotcha |
|----------|----------------|
| **Jira** | ADF description format still mandatory on v3 — requires converter |
| **GitHub** | Dual model (Issues REST + ProjectsV2 GraphQL) multiplies API surface |
| **Linear** | Hard ceiling on custom fields (labels + Asks only); estimate is points, not hours |
| **Monday.com** | Dynamic column schema — adapter must introspect each board before mapping |
| **Asana** | Native priority limited to 3 fixed values; rich status still needs a custom field |
| **ClickUp** | Free/Unlimited/Business = 100 rpm; realistic sync needs Business Plus (1k rpm) |
| **Todoist** | No `status` field beyond done/not-done; no start_date; no logged_hours |
| **Notion** | Description = recursive block tree at 3 req/s; 2025-09-03 Data Source split was breaking |
| **Trello** | No subtasks, no priority, OAuth 1.0a only, no Trello-specific MCP |
| **MS Planner** | `percentComplete` is 0/50/100 only; Graph webhooks don't include Planner; premium plans API-inaccessible |
<!-- END:AUTOGEN_GOTCHAS -->

---

## 6. Maintaining the Scorecard

The scorecard is reviewed **quarterly** (or when a provider ships a material API change). The rubric is reviewed **annually** — see [SCORECARD_RUBRIC.md](SCORECARD_RUBRIC.md).

**Audit process** — see [SCORECARD_RUBRIC.md §6](SCORECARD_RUBRIC.md#6-audit-process). It defines the deterministic steps for re-scoring a provider, diffing against the previous audit, and logging to `docs/scorecard-history.json`.

**Scorecard vs. rubric versions** — score changes bump the scorecard version; rubric changes bump the rubric version. Bumping the **rubric major** triggers a full re-audit because historical comparisons are no longer apples-to-apples.

**Disputing a score** — open a GitHub Issue citing a vendor-doc URL showing the sub-criterion is met (or not). Maintainers re-apply the rubric and, if a cell changes, publish a patch audit.

### Score Version History

Audit log with per-provider deltas lives in [`docs/scorecard-history.json`](scorecard-history.json).
