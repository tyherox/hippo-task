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

Each provider is rated on **8 dimensions** using a 1–5 scale:

| Score | Meaning |
|-------|---------|
| ⭐⭐⭐⭐⭐ (5) | Excellent — best-in-class, no friction |
| ⭐⭐⭐⭐ (4) | Good — works well with minor limitations |
| ⭐⭐⭐ (3) | Adequate — functional but has notable gaps |
| ⭐⭐ (2) | Weak — significant limitations, requires workarounds |
| ⭐ (1) | Poor — major barriers to interoperability |

### The 8 Dimensions

| # | Dimension | What We Measure |
|---|-----------|-----------------|
| 1 | **API Completeness** | Does the API expose full CRUD on tasks? Can we read/write all task fields? Are there gaps (read-only fields, missing endpoints)? |
| 2 | **API Ergonomics** | How pleasant is the API to work with? REST vs GraphQL, pagination style, error messages, documentation quality. |
| 3 | **Schema Flexibility** | Can we map all HippoTask core fields? How many fields require `metadata` overflow? Custom field support? |
| 4 | **Rate Limit Generosity** | How many requests can we make? Is the limit practical for real-world sync? |
| 5 | **Real-Time Capability** | Does the platform support webhooks? How mature is the event system? |
| 6 | **Auth Simplicity** | How easy is it to authenticate? Token-based (simple) vs. complex OAuth with scopes? |
| 7 | **Data Portability** | Can we get all data out? Bulk export? Is there lock-in? Do they respect external IDs? |
| 8 | **AI/Agent Readiness** | Does the platform have features specifically for AI agents? MCP support? Agent APIs? |

The **overall score** is the unweighted average of all 8 dimensions, rounded to one decimal.

---

## 2. The Scorecard

*Verified: March 2026*

| Provider | API Complete | API Ergo | Schema Flex | Rate Limits | Real-Time | Auth Simple | Data Port | AI Ready | **Overall** |
|----------|:-----------:|:--------:|:-----------:|:-----------:|:---------:|:-----------:|:---------:|:--------:|:-----------:|
| **Linear** | 5 | 5 | 3 | 4 | 5 | 5 | 4 | 5 | **4.5** |
| **GitHub** | 4 | 4 | 3 | 4 | 5 | 5 | 5 | 5 | **4.4** |
| **Jira** | 5 | 3 | 5 | 5 | 5 | 3 | 4 | 3 | **4.1** |
| **ClickUp** | 5 | 3 | 5 | 2 | 4 | 4 | 3 | 3 | **3.6** |
| **Asana** | 4 | 4 | 4 | 4 | 4 | 4 | 3 | 2 | **3.6** |
| **Todoist** | 3 | 4 | 2 | 3 | 3 | 5 | 3 | 2 | **3.1** |
| **Monday.com** | 4 | 3 | 4 | 3 | 3 | 4 | 2 | 2 | **3.1** |
| **Trello** | 3 | 3 | 2 | 3 | 4 | 3 | 3 | 2 | **2.9** |
| **MS Planner** | 3 | 2 | 2 | 2 | 3 | 2 | 2 | 2 | **2.3** |
| **Notion** | 3 | 3 | 4 | 1 | 1 | 3 | 2 | 2 | **2.4** |

---

## 3. Detailed Ratings

### 🥇 Linear — Overall: 4.5 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 5 | Full CRUD on issues, projects, labels, cycles, teams. Comprehensive GraphQL coverage. |
| API Ergonomics | 5 | Clean GraphQL API, excellent TypeScript SDK (`@linear/sdk`), introspectable schema, great docs. |
| Schema Flexibility | 3 | No arbitrary custom fields — only labels. Limited extension surface. |
| Rate Limits | 4 | Undocumented but generous in practice. No reported throttling issues. |
| Real-Time | 5 | Comprehensive webhook system covering all entity types. |
| Auth Simplicity | 5 | Personal API keys (one click to generate) + clean OAuth 2.0 flow. |
| Data Portability | 4 | Good API coverage for export. CLI importer for migrations. No explicit export tool. |
| AI/Agent Readiness | 5 | **Agent Sessions API** with lifecycle states (pending, active, error, complete). Purpose-built for AI. |

**Bottom line:** Linear is the most developer-friendly and AI-ready platform. Its only weakness is limited custom fields.

---

### 🥈 GitHub Issues/Projects — Overall: 4.4 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 4 | Full Issues CRUD. ProjectV2 is powerful but separate. No native "task" entity (issues serve as tasks). |
| API Ergonomics | 4 | Good GraphQL + REST APIs. Octokit SDK is mature. Two separate models (Issues vs Projects) adds complexity. |
| Schema Flexibility | 3 | Issues have limited built-in fields. ProjectV2 adds custom fields but requires extra queries. |
| Rate Limits | 4 | 5000 points/hr GraphQL is generous for most use cases. |
| Real-Time | 5 | Best-in-class webhook system. GitHub Actions integration. Extensive event types. |
| Auth Simplicity | 5 | Fine-grained PATs, GitHub Apps, and OAuth all well-documented. |
| Data Portability | 5 | Everything is accessible via API. Git-based storage = inherently portable. Migration tools exist. |
| AI/Agent Readiness | 5 | Agent assignment capabilities (Feb 2026). Copilot integration. Deep CI/CD automation. |

**Bottom line:** GitHub is extremely open and well-documented. The dual model (Issues vs Projects) is the main complexity.

---

### 🥉 Jira — Overall: 4.1 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 5 | The most comprehensive API of any platform. Every field, workflow, and permission is accessible. |
| API Ergonomics | 3 | REST v3 is functional but verbose. ADF for descriptions is painful. Custom field IDs are opaque (`customfield_10042`). |
| Schema Flexibility | 5 | Unlimited custom fields, custom issue types, configurable workflows. Can model anything. |
| Rate Limits | 5 | Generous limits. Bulk operations available for efficiency. |
| Real-Time | 5 | Mature webhook system. JQL for powerful queries. |
| Auth Simplicity | 3 | OAuth 2.0 (3LO) requires Atlassian app registration. API tokens are simple but less capable. |
| Data Portability | 4 | Full API access. Export tools exist. Migration from Jira is well-supported by competitors. |
| AI/Agent Readiness | 3 | No specific AI agent features. Atlassian Intelligence is internal, not API-accessible. |

**Bottom line:** Jira has the deepest API but the worst developer experience. ADF and opaque custom field IDs are major friction points.

---

### ClickUp — Overall: 3.6 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 5 | Full CRUD. Extensive feature coverage including dependencies, time tracking, custom fields. |
| API Ergonomics | 3 | REST API is functional. v2/v3 dual versioning adds confusion. Custom fields use separate endpoint. |
| Schema Flexibility | 5 | Rich custom fields. Deepest hierarchy of any platform (Workspace→Space→Folder→List→Task). |
| Rate Limits | 2 | **100 requests/minute** is very tight. Major sync bottleneck. |
| Real-Time | 4 | Webhooks available for most events. |
| Auth Simplicity | 4 | Simple personal tokens. OAuth also available. |
| Data Portability | 3 | API-accessible but no bulk export. Deep hierarchy means complex extraction. |
| AI/Agent Readiness | 3 | ClickUp Brain is their AI but no developer API for agents. |

**Bottom line:** Feature-rich but rate limits are a serious problem for sync use cases.

---

### Asana — Overall: 3.6 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 4 | Good CRUD. Batch API for some operations. Missing native priority field. |
| API Ergonomics | 4 | Clean REST API. Good SDKs in 5+ languages. `opt_fields` for performance. |
| Schema Flexibility | 4 | Typed custom fields. Tasks in multiple projects. Approval workflow. |
| Rate Limits | 4 | ~1500 req/min is reasonable. |
| Real-Time | 4 | Webhooks on resources. Events API. |
| Auth Simplicity | 4 | PAT and OAuth both straightforward. |
| Data Portability | 3 | API access is good. No built-in bulk export. Tasks spanning multiple projects complicates extraction. |
| AI/Agent Readiness | 2 | No agent-specific features. AI Studio is internal. |

**Bottom line:** Solid, well-designed API. Missing native priority is annoying. No AI agent story.

---

### Todoist — Overall: 3.1 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 3 | Basic CRUD. Can't move tasks between projects in REST API. Limited metadata. |
| API Ergonomics | 4 | Simple REST API. Clean. Easy to use. Good docs. |
| Schema Flexibility | 2 | Labels only for custom data. No custom fields. Very limited extension surface. |
| Rate Limits | 3 | 1000 req/15min is okay for personal use, tight for team sync. |
| Real-Time | 3 | Webhooks available but less mature than enterprise tools. |
| Auth Simplicity | 5 | Bearer token, copy-paste from settings. Simplest auth of all platforms. |
| Data Portability | 3 | Basic API access. Sync API for full account data. No enterprise export. |
| AI/Agent Readiness | 2 | No agent features. Simple API makes it easy to integrate but no dedicated support. |

**Bottom line:** Great for personal task management. Too simple for enterprise interop.

---

### Monday.com — Overall: 3.1 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 4 | GraphQL provides comprehensive access. Batch mutations available. |
| API Ergonomics | 3 | GraphQL is powerful but complex. Column-based schema requires discovery before mapping. Complexity-based rate limits are confusing. |
| Schema Flexibility | 4 | Dynamic columns = very flexible. But schema-per-board makes universal mapping hard. |
| Rate Limits | 3 | 5M complexity points/minute. Hard to predict actual throughput. |
| Real-Time | 3 | Integrations API for webhooks. Less comprehensive than Jira/Linear. |
| Auth Simplicity | 4 | API tokens are simple. OAuth available. |
| Data Portability | 2 | Board-based data model makes cross-board extraction complex. No bulk export API. |
| AI/Agent Readiness | 2 | Monday AI Assistant is internal. No developer-facing agent API. |

**Bottom line:** Powerful but the dynamic schema makes adapter implementation uniquely complex.

---

### Trello — Overall: 2.9 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 3 | Basic CRUD on cards. No native subtasks, no dependencies, no time tracking. |
| API Ergonomics | 3 | Simple REST API but dated. Nested resources can be confusing. |
| Schema Flexibility | 2 | Custom fields only via Power-Up. No native priority or status fields (list position = status). |
| Rate Limits | 3 | 300/10s per key is moderate. 100/10s per token is tighter. |
| Real-Time | 4 | Model-level webhooks work well. |
| Auth Simplicity | 3 | API Key + Token requires two credentials. No modern OAuth by default. |
| Data Portability | 3 | JSON export available. API covers everything. Simple model = easy extraction. |
| AI/Agent Readiness | 2 | New Cloud App Security Requirements (Feb 2026) include AI provisions but no agent API. |

**Bottom line:** Simple but limited. Good for basic Kanban but not for rich task interop.

---

### Notion — Overall: 2.4 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 3 | Database CRUD works. But description is Notion Blocks (complex to map). Recent Data Source split adds complexity. |
| API Ergonomics | 3 | Clean REST API design. But breaking changes (2025-09-03) show instability. Block-based content model is heavy. |
| Schema Flexibility | 4 | Rich property types. Very flexible for structured data. |
| Rate Limits | 1 | **3 requests/second** is brutally low. Worst of all platforms. |
| Real-Time | 1 | **No webhooks.** Polling only. This is the biggest gap. |
| Auth Simplicity | 3 | Internal tokens are easy. OAuth requires Notion app review. |
| Data Portability | 2 | Block-based format is hard to convert. Rate limits make bulk export painful. Breaking API changes. |
| AI/Agent Readiness | 2 | Notion AI is internal. No agent API. MCP connections are third-party. |

**Bottom line:** Notion's rate limits and lack of webhooks make it the hardest platform to build reliable sync for.

---

### MS Planner — Overall: 2.3 / 5

| Dimension | Score | Justification |
|-----------|:-----:|---------------|
| API Completeness | 3 | Basic task CRUD. Limited custom data (6 categories). No rich description. |
| API Ergonomics | 2 | Graph API is powerful but complex. Planner is a small surface within the massive Graph. Documentation scattered. |
| Schema Flexibility | 2 | 6 category labels. No custom fields. `percentComplete` limited to 0/50/100. |
| Rate Limits | 2 | Undocumented dynamic throttling. Hard to plan around. |
| Real-Time | 3 | Graph change notifications work but require Azure AD app setup. |
| Auth Simplicity | 2 | Requires Azure AD app registration. Complex OAuth scope management. Enterprise-focused. |
| Data Portability | 2 | Data is inside Microsoft 365 ecosystem. No standalone export. |
| AI/Agent Readiness | 2 | Microsoft Copilot is internal. Business Scenarios API is relevant but enterprise-only. |

**Bottom line:** Deeply embedded in Microsoft 365. Hard to integrate from outside that ecosystem.

---

## 4. Scorecard Visualization

This is the design for an interactive chart we'll include on the docs site and in the README.

### Radar Chart (Per Provider)

Each provider gets a radar/spider chart showing their 8-dimension profile:

```
                API Completeness
                      5
                     /|\
                    / | \
        AI Ready 4/  |  \3 API Ergonomics
                /    |    \
               /     |     \
    Data Port 3──────2──────4 Schema Flex
               \     |     /
                \    |    /
      Auth Simple\  |  /2 Rate Limits
                  \ | /
                   \|/
                    3
               Real-Time

           (Example: Notion — note the collapsed
            Rate Limits and Real-Time dimensions)
```

### Bar Chart (Comparative)

Side-by-side bars for all providers, sorted by overall score:

```
Overall Score (out of 5.0)

Linear       ████████████████████████████████████████████████ 4.5
GitHub       ███████████████████████████████████████████████  4.4
Jira         ██████████████████████████████████████████       4.1
ClickUp      ████████████████████████████████████             3.6
Asana        ████████████████████████████████████             3.6
Todoist      ███████████████████████████████                  3.1
Monday.com   ███████████████████████████████                  3.1
Trello       ████████████████████████████                     2.9
Notion       ███████████████████████                          2.4
MS Planner   ██████████████████████                           2.3
```

### Implementation Plan

- **Static version** (Milestone 1): Markdown table in this document + README badge per provider
- **Interactive version** (Milestone 5): Chart.js or D3 radar charts on the documentation site
- **Machine-readable**: `scorecard.json` file in the repo with all scores, versioned alongside the docs
- **Badges**: npm README badges like `![Linear: 4.5/5](https://img.shields.io/badge/Linear-4.5%2F5-brightgreen)`

---

## 5. What This Means for Users

### "Which platforms should I prioritize?"

| If your goal is… | Best platforms | Avoid |
|-------------------|---------------|-------|
| Reliable bidirectional sync | Linear, GitHub, Jira | Notion (rate limits), MS Planner (schema) |
| Quick read-only aggregation | Any top 6 | MS Planner (auth complexity) |
| AI agent integration | Linear (Agent Sessions), GitHub (agent assignment) | Notion, MS Planner, Trello |
| Maximum field coverage | Jira, ClickUp (custom fields) | Todoist, Trello (limited fields) |
| Simple setup | Todoist (auth), Linear (DX) | Jira (ADF), Monday (dynamic schema) |

### "What are the gotchas?"

| Provider | Biggest Gotcha |
|----------|---------------|
| **Jira** | ADF description format — requires conversion library |
| **Asana** | No native priority field — must use custom field |
| **Linear** | No custom fields beyond labels — limited extension |
| **ClickUp** | 100 req/min rate limit — batching essential |
| **Trello** | List = status — mapping is fragile if users rearrange lists |
| **Monday.com** | Dynamic column schema — adapter must discover before mapping |
| **Notion** | 3 req/s + no webhooks — slowest platform for sync by far |
| **GitHub** | Dual model (Issues vs ProjectV2) — must decide which to map |
| **Todoist** | Can't move tasks between projects via REST API |
| **MS Planner** | Azure AD setup + undocumented rate limits |

---

## 6. Maintaining the Scorecard

### Update Cadence

The scorecard is reviewed **quarterly** (or when a provider makes a significant API change).

### Scoring Process

1. Check each provider's changelog/docs for API changes
2. Re-run smoke tests for each adapter
3. Update scores if criteria have changed
4. Document the change in the score history

### Score History

Each update is logged in `scorecard-history.json`:

```json
{
  "history": [
    {
      "date": "2026-03-08",
      "version": "1.0.0",
      "changes": "Initial scoring based on March 2026 API research."
    }
  ]
}
```

### Community Input

Scores are open to challenge via GitHub Issues. If a user disagrees with a score, they can open an issue with evidence. The maintainers will evaluate and adjust if warranted.
