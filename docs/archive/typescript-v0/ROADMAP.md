# HippoTask — Roadmap

> Phased delivery plan with milestones, tasks, and acceptance criteria.

---

## Overview

The roadmap is structured in 5 milestones, progressing from foundational schema work through first adapters, MCP integration, adapter expansion, and finally sync maturity. Milestones overlap slightly — the MCP server can begin once the core is stable, and adapter expansion can proceed in parallel.

```
Week  1  2  3  4  5  6  7  8  9  10  11  12  13  14
      ├──── M1: Foundation ────┤
                  ├─────── M2: First Adapters ───────┤
                        ├────── M3: MCP Server ──────┤
                              ├───── M4: Adapter Expansion ──────┤
                                          ├──── M5: Sync & Polish ────┤
```

---

## Milestone 1: Foundation (Weeks 1–3)

> **Goal:** Establish the monorepo, define the core schema in code (Zod), set up the adapter interface, and ship the Quick Start example.

### Tasks

| # | Task | Package | Acceptance Criteria |
|---|------|---------|---------------------|
| 1.1 | Initialize monorepo | root | pnpm workspace, Turborepo, Biome, shared tsconfig (`strict: true` + all strict flags per ENGINEERING.md). `pnpm build` and `pnpm test` work from root. |
| 1.2 | Implement HippoTask Zod schema (TDD) | `@hippotask/core` | **Tests written first.** All types from SCHEMA.md defined as Zod schemas. TS types auto-inferred. ≥90% coverage. |
| 1.3 | Implement HippoProject Zod schema (TDD) | `@hippotask/core` | **Tests written first.** Project schema with validation. |
| 1.4 | Implement supporting types (TDD) | `@hippotask/core` | HippoPerson, HippoCustomField, TaskQuery, PaginatedResult, TaskChangeEvent. |
| 1.5 | Validation utilities (TDD) | `@hippotask/core` | `validate(task)` returns typed `Result<HippoTask, ValidationError>`. `createTask(input)` with smart defaults. Business rules enforced. Helpful error messages (per ENGINEERING.md §7). |
| 1.6 | Utility functions | `@hippotask/core` | UUIDv7 generation, ISO 8601 helpers, deep merge for updates. |
| 1.7 | JSON Schema export | `@hippotask/core` | `hippo-task.schema.json` and `hippo-project.schema.json` generated from Zod via `zod-to-json-schema`. Snapshot tests catch unintended changes. |
| 1.8 | Adapter interface definition | `@hippotask/adapter-common` | `HippoAdapter<T>` interface, `AdapterCapabilities`, `StatusMap`, `PriorityMap` types. Follows ISP — optional methods are optional. |
| 1.9 | Shared adapter utilities (TDD) | `@hippotask/adapter-common` | `BaseFieldMapper`, rate limiter (token bucket), retry with backoff, simple TTL cache. All with tests. |
| 1.10 | Quick Start example | `examples/quick-start` | Standalone project. `npm install && npm start` works. Demonstrates `createTask`, `validate`, JSON Schema. See [EXAMPLES.md](./EXAMPLES.md). |
| 1.11 | CI pipeline | root | GitHub Actions: lint (Biome) → typecheck (tsc) → test (Vitest) → build (tsup) → bundle size check (size-limit) on every PR. |
| 1.12 | Provider Scorecard (static) | `docs/` | Machine-readable `scorecard.json` + Markdown table from PROVIDER_SCORECARD.md. |

### Exit Criteria
- `pnpm install && pnpm build && pnpm test` passes cleanly
- A HippoTask JSON object can be validated at runtime with helpful error messages
- `createTask({ title: "My task" })` produces a complete, valid HippoTask in 1 line
- JSON Schema files are generated and valid
- Quick Start example runs in under 60 seconds
- Adapter interface is defined and documented
- All code adheres to ENGINEERING.md standards (SOLID, TDD, strict TS)

---

## Milestone 2: First Adapters (Weeks 4–7)

> **Goal:** Implement 3 adapters covering different API styles (REST, GraphQL) and description formats (ADF, Markdown).

### Adapter Selection Rationale

| Adapter | Why First | API Style | Description Format | Complexity |
|---------|-----------|-----------|-------------------|------------|
| **Jira** | Most complex enterprise tool — proves the schema handles the hard case | REST v3 | ADF (needs conversion) | High |
| **Linear** | Modern, clean GraphQL API — good contrast to Jira | GraphQL | Markdown (native) | Medium |
| **GitHub** | Dogfooding opportunity — use on this very repo | GraphQL + REST | Markdown (native) | Medium |

### Tasks

| # | Task | Package | Acceptance Criteria |
|---|------|---------|---------------------|
| 2.1 | Jira API client | `@hippotask/adapter-jira` | Authenticated REST client. CRUD operations on issues. Handles pagination. |
| 2.2 | Jira field mapper | `@hippotask/adapter-jira` | Bidirectional mapping: Jira Issue ↔ HippoTask. All core fields mapped. Custom fields flow to `custom_fields`. Unmapped data flows to `metadata`. |
| 2.3 | Jira ADF conversion | `@hippotask/adapter-jira` | Markdown → ADF and ADF → Markdown conversion. Handles common elements (headings, lists, code blocks, links). |
| 2.4 | Jira status/priority map | `@hippotask/adapter-jira` | Default status map (To Do/In Progress/Done categories). Default priority map. Both overridable. |
| 2.5 | Jira adapter integration | `@hippotask/adapter-jira` | Full `HippoAdapter` implementation. Passes contract test suite. |
| 2.6 | Linear GraphQL client | `@hippotask/adapter-linear` | Authenticated GraphQL client. CRUD on issues. Handles cursor-based pagination. |
| 2.7 | Linear field mapper | `@hippotask/adapter-linear` | Bidirectional mapping. Priority inversion handled (Linear 1=Urgent, HippoTask maps correctly). |
| 2.8 | Linear adapter integration | `@hippotask/adapter-linear` | Full `HippoAdapter` implementation. Passes contract test suite. |
| 2.9 | GitHub client | `@hippotask/adapter-github` | Authenticated client for Issues + ProjectV2 API. |
| 2.10 | GitHub field mapper | `@hippotask/adapter-github` | Issue ↔ HippoTask mapping. ProjectV2 custom fields mapped. |
| 2.11 | GitHub adapter integration | `@hippotask/adapter-github` | Full `HippoAdapter` implementation. Passes contract test suite. |
| 2.12 | Contract test suite | `@hippotask/adapter-common` | Generic test suite that runs against any `HippoAdapter` implementation. Tests all interface methods. Validates output against core Zod schemas. |
| 2.13 | Mock server infrastructure | test utilities | Reusable HTTP/GraphQL mock servers for adapter integration tests. |
| 2.14 | Description conversion tests | adapters | Round-trip tests: Markdown → ADF → Markdown preserves content. Markdown → GFM passthrough works. |

### Exit Criteria
- All 3 adapters implement the full `HippoAdapter` interface
- All 3 pass the contract test suite
- Tasks can be round-tripped (create on platform → read back → compare) with minimal data loss
- Description conversion works for common formatting (headings, lists, bold, italic, code, links)

---

## Milestone 3: MCP Server (Weeks 6–9)

> **Goal:** Ship an MCP server that AI agents can use for task management, with optional platform sync.

### Tasks

| # | Task | Package | Acceptance Criteria |
|---|------|---------|---------------------|
| 3.1 | MCP server scaffold | `@hippotask/mcp-server` | Server initializes with `@modelcontextprotocol/sdk`. Registers tools, resources, prompts. Runs on stdio and HTTP transports. |
| 3.2 | In-memory task store | `@hippotask/mcp-server` | `MemoryTaskStore` implements `TaskStore`. Full CRUD + query with filtering/sorting/pagination. |
| 3.3 | File-backed task store | `@hippotask/mcp-server` | `FileTaskStore` reads/writes JSON file. Atomic writes (write-to-temp + rename). |
| 3.4 | CRUD tools | `@hippotask/mcp-server` | `hippotask_create`, `hippotask_get`, `hippotask_list`, `hippotask_update`, `hippotask_delete` tools. All validate input with Zod. All produce valid HippoTask output. |
| 3.5 | Sync tools | `@hippotask/mcp-server` | `hippotask_sync_pull`, `hippotask_sync_push`, `hippotask_connect` tools. Pull/push tasks between store and connected adapters. |
| 3.6 | Resource definitions | `@hippotask/mcp-server` | `hippotask://tasks`, `hippotask://tasks/{id}`, `hippotask://projects`, `hippotask://adapters`, `hippotask://schema` resources. |
| 3.7 | Prompt templates | `@hippotask/mcp-server` | `hippotask_plan_work`, `hippotask_status_report`, `hippotask_triage` prompts with well-crafted templates. |
| 3.8 | Dynamic adapter loading | `@hippotask/mcp-server` | Adapters loaded via `hippotask_connect` tool. Graceful errors if adapter package not installed. |
| 3.9 | MCP client test suite | `@hippotask/mcp-server` | Integration tests using an MCP test client. All tools, resources, prompts tested. |
| 3.10 | Claude Desktop testing | manual | Manual verification with Claude Desktop as MCP host. Document setup instructions. |
| 3.11 | Agent workflow example | `examples/mcp-agent` | Example showing an agent creating tasks, tracking progress, and syncing to GitHub. |

### Exit Criteria
- MCP server runs and connects via stdio (Claude Desktop) and HTTP (web clients)
- An AI agent can perform full task lifecycle (create → list → update → complete → delete)
- Sync pull/push works with at least one adapter (GitHub or Linear)
- Storage persists across server restarts (file store) or is ephemeral (memory store)

---

## Milestone 4: Adapter Expansion (Weeks 8–12)

> **Goal:** Expand platform coverage to all 10 target platforms.

### Tasks

| # | Task | Package | Acceptance Criteria |
|---|------|---------|---------------------|
| 4.1 | Asana adapter | `@hippotask/adapter-asana` | Full implementation. HTML ↔ Markdown conversion. Handles tasks in multiple projects. |
| 4.2 | ClickUp adapter | `@hippotask/adapter-clickup` | Full implementation. Handles deep hierarchy (Space→Folder→List→Task). Priority inversion (1=Urgent). |
| 4.3 | Trello adapter | `@hippotask/adapter-trello` | Full implementation. List-based status mapping. Label mapping. |
| 4.4 | Notion adapter | `@hippotask/adapter-notion` | Full implementation. Notion Blocks ↔ Markdown conversion. Handles Data Source model (2025-09-03 API). Rate limit aware (3 req/s). |
| 4.5 | Monday.com adapter | `@hippotask/adapter-monday` | Full implementation. Dynamic column schema discovery. GraphQL. |
| 4.6 | Todoist adapter | `@hippotask/adapter-todoist` | Full implementation. Simple mapping. Handles limited features gracefully. |
| 4.7 | MS Planner adapter | `@hippotask/adapter-planner` | Full implementation. Graph API. Business Scenarios support. |
| 4.8 | Adapter development guide | `docs/` | Guide for community contributors to build new adapters. Templates, conventions, test requirements. |
| 4.9 | All adapters pass contract tests | all adapters | Every adapter passes the same generic contract test suite. |

### Exit Criteria
- All 10 adapters implement `HippoAdapter` and pass contract tests
- Each adapter has platform-specific integration tests with mock servers
- Adapter development guide enables community contributions

---

## Milestone 5: Distribution, Examples & Polish (Weeks 10–14)

> **Goal:** Mature sync, publish packages, ship the interactive playground, and prepare for public release.

### Tasks

| # | Task | Package | Acceptance Criteria |
|---|------|---------|---------------------|
| 5.1 | Bidirectional sync engine | `@hippotask/adapter-common` | Sync orchestrator that handles pull + push + conflict detection. |
| 5.2 | Conflict resolution | `@hippotask/adapter-common` | Strategies: `last_write_wins`, `local_wins`, `remote_wins`, `manual`. Configurable per adapter. |
| 5.3 | Webhook receiver | `@hippotask/adapter-common` | Generic webhook handler that adapters can implement. Signature verification. |
| 5.4 | CLI tool | new package or root script | `npx hippotask pull jira`, `npx hippotask push linear`, `npx hippotask list`, `npx hippotask validate < task.json`. |
| 5.5 | npm publishing setup | root | Changesets for versioning. Automated publish on release tag. ESM + CJS dual builds per DISTRIBUTION.md. Package READMEs with badges. |
| 5.6 | JSR publishing | root | Publish `@hippotask/core` to JSR for Deno users. |
| 5.7 | Documentation site (Starlight) | `docs-site/` | Starlight (Astro) site deployed to GitHub Pages. Includes: getting started, schema reference (TypeDoc), adapter guides, MCP setup, provider scorecard (interactive charts). |
| 5.8 | Interactive Playground | `examples/playground` | Self-contained Vite + React app. Task editor with live validation, adapter simulator, provider scorecard radar charts, format converter. Per EXAMPLES.md spec. |
| 5.9 | Multi-Platform Aggregator example | `examples/multi-platform` | Working example with mock mode + real adapter mode. Pretty report output. Per EXAMPLES.md spec. |
| 5.10 | MCP Agent Workflow example | `examples/mcp-agent` | Working example with file store + simulated agent. Claude Desktop config template. Per EXAMPLES.md spec. |
| 5.11 | Provider Scorecard (interactive) | docs site | Chart.js radar charts for each provider. Comparative bar chart. Data from `scorecard.json`. |
| 5.12 | Bundle size budgets | root | `size-limit` config enforcing package size targets from DISTRIBUTION.md. CI fails on regression. |
| 5.13 | Public launch checklist | root | LICENSE (MIT), CONTRIBUTING.md, CODE_OF_CONDUCT.md, issue templates, PR template, adapter development guide. |
| 5.14 | Example CI testing | `.github/workflows/` | All examples built and smoke-tested in CI. |

### Exit Criteria
- Bidirectional sync works between HippoTask and at least 2 platforms simultaneously
- All packages published to npm under `@hippotask/*` scope (ESM + CJS)
- `@hippotask/core` published to JSR
- Documentation site is live with interactive provider scorecard
- Interactive playground runs at `examples/playground` with `npm run dev`
- All 4 examples run standalone (clone → install → run in < 60 seconds)
- CLI provides basic utility for non-programmatic use
- Bundle sizes within budgets per DISTRIBUTION.md

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Platform API breaking changes during development | Medium | High | Pin API versions, track changelogs, design adapters for version flexibility |
| ADF/Notion Blocks conversion losing formatting | High | Medium | Accept some loss, preserve originals in metadata, comprehensive round-trip tests |
| Rate limits blocking integration tests | Medium | Medium | Use mock servers for tests, real API tests in separate CI job with credentials |
| MCP protocol changes | Low | High | Use official SDK, track MCP spec changelog |
| Schema needing breaking changes post-1.0 | Medium | High | Extensive validation during Milestones 1-2, `schema_version` field for migration |
| Community adoption — competing with OWL/TaskDef | Medium | Low | Focus on working adapters + MCP (our differentiators), not just schema definition |

---

## End Products Summary

The project produces **three deliverables**, detailed in their respective docs:

| Deliverable | Description | Docs |
|-------------|-------------|------|
| **📦 Library** | npm packages (`@hippotask/*`) importable in Node.js, Deno, Bun, browsers, edge runtimes. Schema + validation + adapters + MCP server. | [DISTRIBUTION.md](./DISTRIBUTION.md) |
| **📊 Provider Scorecard** | Dashboard/chart rating each platform on 8 openness metrics (API completeness, rate limits, AI readiness, etc.) — static Markdown + interactive Chart.js. | [PROVIDER_SCORECARD.md](./PROVIDER_SCORECARD.md) |
| **🎮 Examples & Playground** | 4 self-contained examples including an interactive web playground. Eventually evolves into a dedicated service. | [EXAMPLES.md](./EXAMPLES.md) |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-08 | TypeScript as primary language | Best MCP SDK support, JSON-native, broadest ecosystem for target users |
| 2026-03-08 | Markdown as canonical description format | 6/10 platforms native, mature converters for others |
| 2026-03-08 | Zod for schema validation | Runtime validation + TS types + JSON Schema export in one |
| 2026-03-08 | UUIDv7 for internal IDs | Time-sortable, globally unique, no coordination needed |
| 2026-03-08 | Jira + Linear + GitHub as first 3 adapters | Covers REST + GraphQL, ADF + MD, high complexity + low complexity |
| 2026-03-08 | Adapters as optional peer deps of MCP server | Users install only what they need, keeps bundle small |
| 2026-03-08 | 6-value HippoStatus enum | Covers >95% of workflows, platform-specific statuses preserved in status_raw |
| 2026-03-08 | Flat Project → Task hierarchy | Avoids platform-specific organizational layers, deeper hierarchy preserved in metadata |
| 2026-03-08 | Strict TDD — tests before code | ENGINEERING.md mandates Red-Green-Refactor for all core logic |
| 2026-03-08 | SOLID as non-negotiable | Every principle mapped to concrete HippoTask patterns in ENGINEERING.md |
| 2026-03-08 | Progressive disclosure DX | 2 lines for basic use, 5 for adapters, complexity only when needed |
| 2026-03-08 | npm + JSR dual publishing | npm primary (broadest reach), JSR for Deno (secondary) |
| 2026-03-08 | ESM primary, CJS fallback | Modern default, backwards compatibility via tsup |
| 2026-03-08 | Starlight (Astro) for docs site | Purpose-built for OSS docs, search, versioning, GitHub Pages deployment |
| 2026-03-08 | Provider scorecard with 8 metrics | Objective, measurable dimensions. Quarterly review cadence. |
| 2026-03-08 | Interactive playground as Phase 1 of service | Self-contained Vite+React app, browser-only, evolves into hosted service later |
