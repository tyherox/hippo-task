# HippoTask — Tech Specs

> Verified API landscape, platform comparison, field mapping, and technology decisions.
>
> *Last verified: March 2026*

---

## Table of Contents

- [1. Platform API Landscape](#1-platform-api-landscape)
- [2. Universal Field Mapping Matrix](#2-universal-field-mapping-matrix)
- [3. Rate Limits & Sync Constraints](#3-rate-limits--sync-constraints)
- [4. Description Format Matrix](#4-description-format-matrix)
- [5. Authentication Summary](#5-authentication-summary)
- [6. Technology Stack Decisions](#6-technology-stack-decisions)

---

## 1. Platform API Landscape

### 1.1 Jira (Atlassian Cloud)

| Property | Detail |
|----------|--------|
| **API** | REST API v3 |
| **Base URL** | `https://your-domain.atlassian.net/rest/api/3/` |
| **Auth** | OAuth 2.0 (3LO), API Token (Basic Auth) |
| **Core Entity** | Issue (types: Task, Bug, Story, Epic, Subtask) |
| **Description** | Atlassian Document Format (ADF) — a JSON-based rich text format |
| **Custom Fields** | Unlimited, dynamic, typed (`customfield_10001`, etc.) |
| **Hierarchy** | Site → Project → Issue → Subtask |
| **Webhooks** | Yes — issue created/updated/deleted, project events |
| **Bulk Operations** | Yes — bulk create, update, delete, transition |
| **SDKs** | Official REST client, community TypeScript packages |

**Key observations:**
- ADF is Jira's proprietary description format. A Markdown ↔ ADF converter is required.
- Custom fields use numeric IDs (`customfield_NNNNN`), requiring a field metadata lookup to map meaningfully.
- Jira's status model uses a workflow/transition system — statuses belong to status categories (To Do, In Progress, Done) which simplifies mapping.
- The v3 API is the current standard for Jira Cloud; Server/Data Center uses older API versions.

### 1.2 Asana

| Property | Detail |
|----------|--------|
| **API** | REST API v1 |
| **Base URL** | `https://app.asana.com/api/1.0/` |
| **Auth** | OAuth 2.0, Personal Access Token |
| **Core Entity** | Task (subtypes: `default_task`, `milestone`, `approval`) |
| **Description** | HTML (`html_notes` field) or plain text (`notes`) |
| **Custom Fields** | Yes — typed (text, number, enum, multi_enum, date, people) |
| **Hierarchy** | Workspace → Team → Project → Section → Task → Subtask |
| **Webhooks** | Yes — resource-level (tasks, projects, etc.) |
| **Bulk Operations** | Limited — batch API for some operations |
| **SDKs** | Official Node, Python, Ruby, Java, PHP SDKs |

**Key observations:**
- Asana tasks can live in multiple projects simultaneously — our `project_id` maps to the first/primary project.
- Asana has no native "priority" field — priority must be modeled as a custom field.
- The approval subtype adds `approval_status` (pending, approved, rejected, changes_requested).
- Asana's API uses `opt_fields` for field selection — important for performance.

### 1.3 Linear

| Property | Detail |
|----------|--------|
| **API** | GraphQL |
| **Endpoint** | `https://api.linear.app/graphql` |
| **Auth** | OAuth 2.0, Personal API Key |
| **Core Entity** | Issue |
| **Description** | Markdown |
| **Custom Fields** | Labels (not arbitrary custom fields) |
| **Hierarchy** | Workspace → Team → Project → Issue → Sub-issue |
| **Webhooks** | Yes — comprehensive event types |
| **Bulk Operations** | Via GraphQL mutations |
| **SDKs** | Official TypeScript SDK (`@linear/sdk`) |

**Key observations:**
- Linear's API is GraphQL-only, which means our adapter must construct queries instead of REST calls.
- Priority is a numeric scale: 0 (No priority), 1 (Urgent), 2 (High), 3 (Medium), 4 (Low).
- Linear has introduced **Agent Sessions** (2025+) — a lifecycle model for AI agents with states: `pending`, `active`, `error`, `awaitingInput`, `complete`. This is directly relevant to UC3.
- Cursor-based Relay-style pagination.
- Linear is opinionated with limited custom field support — `metadata` overflow will be important.

### 1.4 ClickUp

| Property | Detail |
|----------|--------|
| **API** | REST API v2 (stable), v3 (newer) |
| **Base URL** | `https://api.clickup.com/api/v2/` |
| **Auth** | OAuth 2.0, Personal API Token |
| **Core Entity** | Task |
| **Description** | Markdown |
| **Custom Fields** | Yes — typed (drop down, labels, text, number, date, etc.) |
| **Hierarchy** | Workspace → Space → Folder → List → Task → Subtask → Checklist |
| **Webhooks** | Yes — task, list, folder, space events |
| **Bulk Operations** | Limited |
| **SDKs** | Community packages |

**Key observations:**
- ClickUp has the deepest hierarchy of all platforms — mapping to our flat Project/Task model will require decisions about which level maps to `project_id`.
- Priority scale: 1 (Urgent), 2 (High), 3 (Normal), 4 (Low) — note: reversed from Linear.
- v2 uses "Team" for what v3 calls "Workspace" — adapter must handle both.
- Custom fields are updated via a separate endpoint from task updates.
- Time estimates are in milliseconds.

### 1.5 Trello (Atlassian)

| Property | Detail |
|----------|--------|
| **API** | REST API |
| **Base URL** | `https://api.trello.com/1/` |
| **Auth** | API Key + User Token |
| **Core Entity** | Card |
| **Description** | Markdown |
| **Custom Fields** | Via Custom Fields Power-Up |
| **Hierarchy** | Board → List → Card → Checklist → CheckItem |
| **Webhooks** | Yes — model-level (board, card, etc.) |
| **Bulk Operations** | No native bulk API |
| **SDKs** | Community packages |

**Key observations:**
- Trello uses Lists as status columns — the list a card belongs to effectively determines its status.
- No native priority field — must be modeled via labels or custom fields.
- Trello's model is simple but limited for complex workflows.
- Rate limits are tight: 300 req/10s per API key, 100 req/10s per token.
- New Cloud App Security Requirements (Feb 2026) include AI security provisions.

### 1.6 Monday.com

| Property | Detail |
|----------|--------|
| **API** | GraphQL |
| **Endpoint** | `https://api.monday.com/v2` |
| **Auth** | OAuth 2.0, API Token |
| **Core Entity** | Item (within a Board) |
| **Description** | Markdown (via `set_item_description_content` mutation) |
| **Custom Fields** | Column-based — extremely flexible, schema-per-board |
| **Hierarchy** | Workspace → Board → Group → Item → Subitem |
| **Webhooks** | Yes — via integrations API |
| **Bulk Operations** | Via `create_items_batch` mutation |
| **SDKs** | Official JavaScript SDK |

**Key observations:**
- Monday.com is the most dynamically-typed platform — boards define their own column schemas.
- The adapter must discover a board's column schema before mapping fields.
- Latest API version: `2026-04` — versioned with advance deprecation notices.
- Complexity-based rate limiting (5M points/minute) rather than request-based.
- Status is a column type, not a fixed field.

### 1.7 Notion

| Property | Detail |
|----------|--------|
| **API** | REST API (version `2025-09-03`) |
| **Base URL** | `https://api.notion.com/v1/` |
| **Auth** | OAuth 2.0, Internal Integration Token |
| **Core Entity** | Page (within a Database/Data Source) |
| **Description** | Notion Blocks (proprietary rich text JSON) |
| **Custom Fields** | Property-based — fully typed (title, text, number, select, date, people, checkbox, url, email, relation, formula, rollup, status) |
| **Hierarchy** | Workspace → Page/Database → Data Source → Page |
| **Webhooks** | Not available — polling required |
| **Bulk Operations** | Not available |
| **SDKs** | Official JavaScript SDK (`@notionhq/client`) |

**Key observations:**
- **MAJOR BREAKING CHANGE (2025-09-03):** Databases split into Database (container) + Data Source (individual schemas). All integrations must use `data_source_id` instead of `database_id` for read/write.
- Notion Blocks are their own rich text format — not Markdown, not HTML. Conversion libraries exist.
- Rate limit is very low: 3 requests per second. Adapter must implement aggressive caching and batching.
- No webhooks — must poll for changes. This limits real-time sync capability.
- Notion is property-flexible but structurally different from traditional task tools.

### 1.8 GitHub Issues / Projects

| Property | Detail |
|----------|--------|
| **API** | GraphQL + REST v3 |
| **Endpoint** | `https://api.github.com/graphql` |
| **Auth** | Personal Access Token, GitHub App, Fine-grained PAT |
| **Core Entity** | Issue + ProjectV2Item |
| **Description** | GitHub-Flavored Markdown |
| **Custom Fields** | ProjectV2 fields (text, number, date, single_select, iteration) |
| **Hierarchy** | Organization → Repository → Issue; Organization → ProjectV2 → ProjectV2Item |
| **Webhooks** | Yes — comprehensive event system |
| **Bulk Operations** | GraphQL batch mutations |
| **SDKs** | Octokit (JS/TS), PyGitHub, go-github |

**Key observations:**
- Two distinct models: Issues (repo-level) and Projects (cross-repo). Adapter should support both.
- Status in Projects is a custom single_select field, not a fixed enum.
- Agent assignment capabilities added Feb 2026 — directly relevant to UC3.
- GitHub's API has points-based rate limiting (5000 points/hour for authenticated users).
- Markdown is native — no conversion needed.

### 1.9 Todoist

| Property | Detail |
|----------|--------|
| **API** | REST API v2 + Sync API |
| **Base URL** | `https://api.todoist.com/rest/v2/` |
| **Auth** | OAuth 2.0, Bearer Token |
| **Core Entity** | Task |
| **Description** | Markdown (limited subset) |
| **Custom Fields** | Labels only |
| **Hierarchy** | Project → Section → Task → Subtask |
| **Webhooks** | Yes — app webhooks |
| **Bulk Operations** | Via Sync API |
| **SDKs** | Official Python (`todoist-api-python`), TypeScript (`@doist/todoist-api-typescript`) |

**Key observations:**
- Simpler than enterprise tools — good for personal task management use case.
- Priority scale: 1 (natural/none in UI) through 4 (urgent) — but the API inverts the display.
- Moving tasks between projects not yet supported in REST API — requires Sync API or recreation.
- Limited custom data — mostly labels.
- Sync API provides incremental sync for full-account data — useful for UC2.

### 1.10 Microsoft Planner (Graph API)

| Property | Detail |
|----------|--------|
| **API** | Microsoft Graph API |
| **Endpoint** | `https://graph.microsoft.com/v1.0/planner/` |
| **Auth** | OAuth 2.0 (delegated + application permissions) |
| **Core Entity** | PlannerTask |
| **Description** | Plain text (in task details) |
| **Custom Fields** | 6 category labels + Business Scenarios |
| **Hierarchy** | Group/Team → Plan → Bucket → Task |
| **Webhooks** | Via Graph change notifications (subscriptions) |
| **Bulk Operations** | Not natively supported |
| **SDKs** | Microsoft Graph SDK for JS, Python, .NET, Java |

**Key observations:**
- Tightly coupled to Microsoft 365 / Teams ecosystem.
- Application permissions support (added 2023) enables automated workflows without signed-in users.
- Business Scenarios (newer) allow external system integration with external IDs.
- `percentComplete` (0, 50, 100) is the progress model — very coarse.
- 6 appliedCategories serve as fixed labels.

---

## 2. Universal Field Mapping Matrix

This matrix shows how each HippoTask field maps to every supported platform. A `—` indicates the platform has no native equivalent (the value would be stored in `metadata` or `custom_fields`).

### Core Fields

| HippoTask | Jira | Asana | Linear | ClickUp | Trello | Monday | Notion | GitHub | Todoist | MS Planner |
|-----------|------|-------|--------|---------|--------|--------|--------|--------|---------|------------|
| `id` | `key` | `gid` | `identifier` | `id` | `id` | `id` | `id` | `number` | `id` | `id` |
| `title` | `summary` | `name` | `title` | `name` | `name` | `name` (col) | `Name` (prop) | `title` | `content` | `title` |
| `description` | `description` (ADF) | `html_notes` | `description` (MD) | `description` (MD) | `desc` (MD) | desc (MD) | blocks | `body` (GFM) | `description` (MD) | `details.description` |
| `status` | `status.name` | completion + section | `state.name` | `status.status` | `idList` (list name) | Status column | Status prop | `state` | `is_completed` | `percentComplete` |
| `assignees` | `assignee` (single) | `assignee` (single) | `assignee` (single) | `assignees[]` | `idMembers[]` | People column | People prop | `assignees[]` | `responsible_uid` | `assignments{}` |
| `priority` | `priority.name` | — (custom) | `priority` (0-4) | `priority` (1-4) | — (label) | Priority column | Select prop | — (label) | `priority` (1-4) | `priority` (0-10) |
| `due_date` | `duedate` | `due_on` / `due_at` | `dueDate` | `due_date` | `due` | Date column | Date prop | — (milestone) | `due.date` | `dueDateTime` |
| `start_date` | — (custom) | `start_on` / `start_at` | — | `start_date` | `start` | Date column | Date prop | — | — | `startDateTime` |
| `created_at` | `created` | `created_at` | `createdAt` | `date_created` | `dateLastActivity` | `created_at` | `created_time` | `created_at` | `created_at` | `createdDateTime` |
| `updated_at` | `updated` | `modified_at` | `updatedAt` | `date_updated` | `dateLastActivity` | — | `last_edited_time` | `updated_at` | — | — |
| `completed_at` | `resolutiondate` | `completed_at` | `completedAt` | `date_closed` | — | — | — | `closed_at` | `completed_at` | — |
| `labels` | `labels[]` | `tags[]` | `labels[]` | `tags[]` | `labels[]` | Tags column | Multi-select prop | `labels[]` | `labels[]` | `appliedCategories` |
| `parent_id` | `parent.key` | `parent.gid` | `parent.id` | `parent` | — | — (subitems) | — (relation) | — | `parent_id` | — |
| `project_id` | `project.key` | `projects[0].gid` | `project.id` | `list.id` | `idBoard` | `board.id` | `database_id` | repo / ProjectV2 | `project_id` | `planId` |
| `creator` | `reporter` | `created_by` | `creator` | `creator` | — | — | `created_by` | `user` (author) | — | `createdBy` |
| `estimate` | `timeoriginalestimate` | — (custom) | `estimate` | `time_estimate` | — | Number column | Number prop | — | — | — |

### Priority Normalization

| HippoTask | Jira | Linear | ClickUp | Todoist | MS Planner |
|-----------|------|--------|---------|---------|------------|
| `urgent` | Highest / Blocker | 1 | 1 | 4 | 0-1 |
| `high` | High | 2 | 2 | 3 | 2-3 |
| `medium` | Medium | 3 | 3 | 2 | 4-5 |
| `low` | Low | 4 | 4 | 1 | 6-9 |
| `none` | — | 0 | — | — | 10 |

### Status Normalization

| HippoTask | Jira Category | Asana | Linear | ClickUp | Trello | GitHub |
|-----------|---------------|-------|--------|---------|--------|--------|
| `backlog` | To Do | section-based | Backlog | — | first list | — |
| `todo` | To Do | not completed, no section | Todo | to do | — | `open` |
| `in_progress` | In Progress | not completed, section | In Progress | in progress | middle lists | `open` + assigned |
| `in_review` | In Progress | — | In Review | — | — | — |
| `done` | Done | `completed=true` | Done | closed | last list / `closed=true` | `closed` |
| `cancelled` | Done (resolution) | `completed=true` (custom) | Cancelled | closed | `closed=true` | `closed` + label |

---

## 3. Rate Limits & Sync Constraints

| Platform | Rate Limit | Constraint Model | Sync Impact |
|----------|-----------|------------------|-------------|
| **Jira** | Varies by plan (typically ~100 req/s) | Request-based | Low impact — generous limits |
| **Asana** | ~1500 req/min | Request-based | Moderate — use `opt_fields` to reduce calls |
| **Linear** | Undocumented (generous) | Request-based | Low impact |
| **ClickUp** | 100 req/min | Request-based | **High impact** — requires careful batching |
| **Trello** | 300/10s (key), 100/10s (token) | Dual rate limit | Moderate — respect both limits |
| **Monday.com** | 5M complexity points/min | Complexity-based | Moderate — optimize query complexity |
| **Notion** | 3 req/s | Request-based | **Very high impact** — aggressive caching needed |
| **GitHub** | 5000 pts/hr (GraphQL), 5000 req/hr (REST) | Points + request-based | Moderate |
| **Todoist** | 1000 req/15min | Request-based | Moderate |
| **MS Planner** | Throttled (undocumented) | Dynamic throttling | Moderate — use batch requests |

### Sync Strategy Implications

- **Webhook-first platforms** (Jira, Asana, Linear, ClickUp, Trello, GitHub, Todoist): Use webhooks for real-time push notifications, fall back to polling.
- **Polling-only platforms** (Notion): Must implement efficient `updated_at`-based polling with caching.
- **Tight rate limits** (Notion, ClickUp): Adapter must implement request queuing, retry with backoff, and local caching.

---

## 4. Description Format Matrix

| Platform | Native Format | Conversion Needed | Library/Approach |
|----------|--------------|-------------------|------------------|
| **Jira** | ADF (Atlassian Document Format) | Markdown ↔ ADF | `adf-builder`, `@atlaskit/adf-utils`, or custom converter |
| **Asana** | HTML | Markdown ↔ HTML | `marked` (MD→HTML), `turndown` (HTML→MD) |
| **Linear** | Markdown | None (native) | — |
| **ClickUp** | Markdown | None (native) | — |
| **Trello** | Markdown | None (native) | — |
| **Monday.com** | Markdown (via mutation) | None (native) | — |
| **Notion** | Notion Blocks (JSON) | Markdown ↔ Notion Blocks | `@notionhq/notion-to-md`, `notion-to-md` |
| **GitHub** | GitHub-Flavored Markdown | Minimal (GFM quirks) | Nearly passthrough |
| **Todoist** | Limited Markdown | Minimal | Nearly passthrough |
| **MS Planner** | Plain text | Markdown → plain text (lossy) | Strip formatting |

**Canonical format decision:** **Markdown** is the clear winner. 6 of 10 platforms use it natively. The remaining 4 have mature conversion libraries. HippoTask stores descriptions as Markdown and converts on adapter boundaries.

---

## 5. Authentication Summary

| Platform | OAuth 2.0 | API Key/Token | App-level Auth | Recommended for HippoTask |
|----------|-----------|---------------|----------------|---------------------------|
| **Jira** | ✅ (3-legged) | ✅ (Basic + token) | ✅ (Connect apps) | OAuth 2.0 for multi-user, API token for personal |
| **Asana** | ✅ | ✅ (PAT) | — | OAuth 2.0 for apps, PAT for scripts |
| **Linear** | ✅ | ✅ (Personal API Key) | — | OAuth 2.0 for apps, PAK for scripts |
| **ClickUp** | ✅ | ✅ (Personal token) | — | OAuth 2.0 for apps, token for personal |
| **Trello** | — | ✅ (API Key + Token) | ✅ (Power-Ups) | API Key + Token |
| **Monday.com** | ✅ | ✅ (API Token) | — | OAuth 2.0 for apps, token for personal |
| **Notion** | ✅ | ✅ (Internal token) | — | OAuth 2.0 for public, internal for personal |
| **GitHub** | ✅ | ✅ (PAT / Fine-grained) | ✅ (GitHub Apps) | GitHub App for apps, PAT for scripts |
| **Todoist** | ✅ | ✅ (Bearer token) | — | OAuth 2.0 for apps, bearer for personal |
| **MS Planner** | ✅ | — | ✅ (App permissions) | OAuth 2.0 (delegated or app) |

**Adapter auth strategy:** Each adapter accepts a config object with platform-specific credentials. HippoTask does NOT manage OAuth flows — it accepts already-obtained tokens. A separate `@hippotask/auth-helpers` package could provide OAuth flow utilities in the future.

---

## 6. Technology Stack Decisions

### Language: TypeScript

| Factor | TypeScript | Python | Go |
|--------|-----------|--------|-----|
| MCP SDK support | ✅ Official `@modelcontextprotocol/sdk` | ✅ Official | ❌ Community |
| JSON-native | ✅ First-class | ✅ Good (dicts) | ⚠️ Struct tags |
| Schema validation | ✅ Zod (runtime + types) | ✅ Pydantic | ⚠️ Manual |
| npm ecosystem | ✅ Native | ❌ PyPI | ❌ Go modules |
| Web/Node.js | ✅ Native | ⚠️ Via WASM | ⚠️ Via WASM |
| AI tool ecosystem | ✅ Dominant | ✅ Strong | ⚠️ Niche |
| Type safety | ✅ Strong | ⚠️ Optional (mypy) | ✅ Strong |

**Decision: TypeScript.** Best MCP support, JSON-native, Zod for schema validation, and the broadest ecosystem for our target users (web developers, AI tool builders).

### Core Dependencies

| Purpose | Package | Rationale |
|---------|---------|-----------|
| Schema validation | `zod` | Runtime validation + TypeScript types + JSON Schema export via `zod-to-json-schema` |
| Build | `tsup` | Simple, fast ESM/CJS dual builds |
| Test | `vitest` | Fast, TS-native, great DX |
| Monorepo | `pnpm` workspaces | Efficient, proven at scale |
| Task runner | `turbo` (Turborepo) | Caching, parallel builds, well-matched with pnpm |
| Linting/Formatting | `biome` | Fast, zero-config, replaces ESLint + Prettier |
| MCP | `@modelcontextprotocol/sdk` | Official SDK |
| HTTP client | `undici` or platform SDKs | Modern fetch-based HTTP, or use official SDKs where available |
| ID generation | `uuidv7` or `nanoid` | UUIDv7 for time-sortable IDs, nanoid for shorter IDs |

### Package Publishing

- Scope: `@hippotask/*`
- Registry: npm (public)
- Format: ESM primary, CJS fallback via `tsup`
- Target: Node.js 20+, modern browsers (for core schema)

### Repository Structure

- Monorepo with pnpm workspaces
- Each package is independently versioned and publishable
- Shared TypeScript config at root
- CI via GitHub Actions
