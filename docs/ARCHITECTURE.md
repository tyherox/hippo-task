# HippoTask — Architecture

> Package structure, adapter interface, MCP server design, sync patterns, and error handling.
>
> For SOLID principles, TDD strategy, and code quality standards, see **[ENGINEERING.md](./ENGINEERING.md)**.
> For distribution, publishing, and multi-environment support, see **[DISTRIBUTION.md](./DISTRIBUTION.md)**.

---

## Table of Contents

- [1. Monorepo Structure](#1-monorepo-structure)
- [2. Package Dependency Graph](#2-package-dependency-graph)
- [3. Adapter Interface](#3-adapter-interface)
- [4. MCP Server Design](#4-mcp-server-design)
- [5. Sync Patterns](#5-sync-patterns)
- [6. Error Handling](#6-error-handling)
- [7. Testing Strategy](#7-testing-strategy)
- [8. Consumer Ease-of-Use Patterns](#8-consumer-ease-of-use-patterns)

---

## 1. Monorepo Structure

```
hippo-task/
│
├── packages/
│   │
│   ├── core/                          @hippotask/core
│   │   ├── src/
│   │   │   ├── index.ts               # Public API exports
│   │   │   ├── schema/
│   │   │   │   ├── task.ts            # HippoTask Zod schema + TS type
│   │   │   │   ├── project.ts         # HippoProject Zod schema + TS type
│   │   │   │   ├── person.ts          # HippoPerson Zod schema + TS type
│   │   │   │   ├── custom-field.ts    # HippoCustomField Zod schema + TS type
│   │   │   │   ├── enums.ts           # Status, Priority, etc.
│   │   │   │   ├── query.ts           # TaskQuery, PaginatedResult
│   │   │   │   └── events.ts          # TaskChangeEvent
│   │   │   ├── validation/
│   │   │   │   ├── validate.ts        # validate(task) → Result<HippoTask, ValidationError[]>
│   │   │   │   └── rules.ts           # Business rules (date ordering, etc.)
│   │   │   └── utils/
│   │   │       ├── id.ts              # UUIDv7 generation
│   │   │       ├── dates.ts           # ISO 8601 helpers
│   │   │       └── merge.ts           # Deep merge for task updates
│   │   ├── schema/                    # Generated JSON Schema files
│   │   │   ├── hippo-task.schema.json
│   │   │   └── hippo-project.schema.json
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   └── tsup.config.ts
│   │
│   ├── adapter-common/                @hippotask/adapter-common
│   │   ├── src/
│   │   │   ├── index.ts
│   │   │   ├── adapter.ts            # HippoAdapter interface
│   │   │   ├── mapper.ts             # BaseFieldMapper class
│   │   │   ├── status-map.ts         # StatusMap type + helpers
│   │   │   ├── priority-map.ts       # PriorityMap type + helpers
│   │   │   ├── description.ts        # Format conversion dispatch
│   │   │   ├── rate-limiter.ts       # Token bucket rate limiter
│   │   │   ├── retry.ts              # Exponential backoff retry
│   │   │   └── cache.ts              # Simple TTL cache for rate-limited platforms
│   │   └── package.json
│   │
│   ├── adapter-jira/                  @hippotask/adapter-jira
│   │   ├── src/
│   │   │   ├── index.ts
│   │   │   ├── client.ts             # Jira REST API client
│   │   │   ├── mapper.ts             # Jira ↔ HippoTask field mapping
│   │   │   ├── adapter.ts            # HippoAdapter implementation
│   │   │   ├── status-map.ts         # Default Jira status mapping
│   │   │   ├── adf.ts                # Markdown ↔ ADF conversion
│   │   │   └── types.ts              # Jira-specific types
│   │   └── package.json
│   │
│   ├── adapter-asana/                 @hippotask/adapter-asana
│   ├── adapter-linear/                @hippotask/adapter-linear
│   ├── adapter-clickup/               @hippotask/adapter-clickup
│   ├── adapter-trello/                @hippotask/adapter-trello
│   ├── adapter-github/                @hippotask/adapter-github
│   ├── adapter-notion/                @hippotask/adapter-notion
│   ├── adapter-monday/                @hippotask/adapter-monday
│   ├── adapter-todoist/               @hippotask/adapter-todoist
│   ├── adapter-planner/               @hippotask/adapter-planner
│   │   └── (same structure as adapter-jira)
│   │
│   └── mcp-server/                    @hippotask/mcp-server
│       ├── src/
│       │   ├── index.ts               # Entry point
│       │   ├── server.ts              # MCP server setup + transport config
│       │   ├── tools/
│       │   │   ├── index.ts           # Tool registry
│       │   │   ├── create-task.ts
│       │   │   ├── get-task.ts
│       │   │   ├── list-tasks.ts
│       │   │   ├── update-task.ts
│       │   │   ├── delete-task.ts
│       │   │   ├── sync-pull.ts
│       │   │   ├── sync-push.ts
│       │   │   └── connect-adapter.ts
│       │   ├── resources/
│       │   │   ├── index.ts           # Resource registry
│       │   │   ├── tasks.ts
│       │   │   ├── projects.ts
│       │   │   ├── adapters.ts
│       │   │   └── schema.ts
│       │   ├── prompts/
│       │   │   ├── index.ts           # Prompt registry
│       │   │   ├── plan-work.ts
│       │   │   ├── status-report.ts
│       │   │   └── triage.ts
│       │   └── storage/
│       │       ├── interface.ts       # TaskStore interface
│       │       ├── memory.ts          # In-memory store (default for agents)
│       │       └── file.ts            # JSON file store (persistent)
│       └── package.json
│
├── docs/                              # This directory (planning docs)
│   ├── VISION.md
│   ├── TECH_SPECS.md
│   ├── SCHEMA.md
│   ├── ARCHITECTURE.md
│   └── ROADMAP.md
│
├── examples/
│   ├── basic-usage/                   # Core schema + validation example
│   ├── jira-sync/                     # Jira adapter example
│   ├── multi-platform/                # Multiple adapters in one app
│   └── mcp-agent/                     # MCP server + AI agent example
│
├── .github/
│   └── workflows/
│       ├── ci.yml                     # Lint, test, build on PR
│       └── release.yml                # Publish to npm on tag
│
├── pnpm-workspace.yaml
├── turbo.json
├── biome.json
├── tsconfig.base.json
├── package.json
└── README.md
```

---

## 2. Package Dependency Graph

```
@hippotask/core
  ├── (no internal deps)
  └── zod

@hippotask/adapter-common
  ├── @hippotask/core
  └── zod

@hippotask/adapter-jira
  ├── @hippotask/core
  ├── @hippotask/adapter-common
  └── (Jira-specific: md-to-adf, adf-to-md, or custom)

@hippotask/adapter-linear
  ├── @hippotask/core
  ├── @hippotask/adapter-common
  └── @linear/sdk (optional — or raw GraphQL)

@hippotask/adapter-github
  ├── @hippotask/core
  ├── @hippotask/adapter-common
  └── @octokit/graphql

... (same pattern for all adapters)

@hippotask/mcp-server
  ├── @hippotask/core
  ├── @hippotask/adapter-common
  ├── @modelcontextprotocol/sdk
  └── zod
  (adapters are optional peer dependencies — loaded dynamically)
```

**Key design decision:** Adapters are **not** bundled dependencies of the MCP server. They are optional peer dependencies or dynamically imported. Users install only the adapters they need:

```bash
# Example: Use HippoTask with Jira and Linear
npm install @hippotask/core @hippotask/mcp-server @hippotask/adapter-jira @hippotask/adapter-linear
```

---

## 3. Adapter Interface

### Core Interface

```typescript
/**
 * Base configuration that all adapters share.
 */
interface BaseAdapterConfig {
  /** Status mapping overrides. */
  statusMap?: Partial<StatusMap>;

  /** Priority mapping overrides. */
  priorityMap?: Partial<PriorityMap>;

  /** Request timeout in ms. Default: 30000. */
  timeout?: number;

  /** Maximum retries on transient failures. Default: 3. */
  maxRetries?: number;
}

/**
 * The adapter interface. Every platform adapter implements this.
 * TConfig is the platform-specific configuration type.
 */
interface HippoAdapter<TConfig extends BaseAdapterConfig = BaseAdapterConfig> {
  /** Platform identifier string. */
  readonly platform: string;

  /** Whether this adapter is currently connected. */
  readonly connected: boolean;

  // ─── Lifecycle ────────────────────────────────────────────
  /** Initialize the adapter with credentials and configuration. */
  connect(config: TConfig): Promise<void>;

  /** Clean up connections and resources. */
  disconnect(): Promise<void>;

  // ─── Task CRUD ────────────────────────────────────────────
  /** Get a single task by its external (platform) ID. */
  getTask(externalId: string): Promise<HippoTask>;

  /** List tasks with optional filters. */
  listTasks(query?: TaskQuery): Promise<PaginatedResult<HippoTask>>;

  /** Create a task on the platform. Returns the task with external_ids populated. */
  createTask(task: HippoTaskCreate): Promise<HippoTask>;

  /** Update a task on the platform. Partial updates supported. */
  updateTask(externalId: string, updates: HippoTaskUpdate): Promise<HippoTask>;

  /** Delete a task on the platform. */
  deleteTask(externalId: string): Promise<void>;

  // ─── Project CRUD ─────────────────────────────────────────
  /** Get a single project by its external ID. */
  getProject(externalId: string): Promise<HippoProject>;

  /** List all accessible projects. */
  listProjects(): Promise<HippoProject[]>;

  // ─── Sync ─────────────────────────────────────────────────
  /**
   * Get tasks that changed since a given timestamp.
   * Used for polling-based sync.
   */
  getChangedTasks(since: string): Promise<HippoTask[]>;

  /**
   * Subscribe to real-time task changes.
   * Not all platforms support this (check `capabilities`).
   */
  onTaskChange?(callback: (event: TaskChangeEvent) => void): Promise<() => void>;

  // ─── Capabilities ─────────────────────────────────────────
  /** What this adapter supports. */
  readonly capabilities: AdapterCapabilities;

  // ─── Mapping ──────────────────────────────────────────────
  /** Get the current status map. */
  getStatusMap(): StatusMap;

  /** Override the default status map. */
  setStatusMap(map: Partial<StatusMap>): void;
}

/**
 * Declares what features an adapter supports.
 * Consumers can check before calling optional methods.
 */
interface AdapterCapabilities {
  /** Can receive real-time change events. */
  webhooks: boolean;

  /** Supports creating tasks. */
  create: boolean;

  /** Supports updating tasks. */
  update: boolean;

  /** Supports deleting tasks. */
  delete: boolean;

  /** Supports subtasks (parent_id). */
  subtasks: boolean;

  /** Supports custom fields. */
  customFields: boolean;

  /** Supports file attachments. */
  attachments: boolean;

  /** Supports bulk operations. */
  bulk: boolean;
}
```

### Adapter Implementation Pattern

Each adapter follows this internal structure:

```
adapter-{platform}/
├── src/
│   ├── index.ts          # Exports: adapter class, config type, default status map
│   ├── adapter.ts         # Main class implementing HippoAdapter<PlatformConfig>
│   ├── client.ts          # HTTP/GraphQL client for the platform API
│   ├── mapper.ts          # Bidirectional field mapping logic
│   │   ├── toHippo(platformTask)  → HippoTask
│   │   └── fromHippo(hippoTask)   → PlatformTask
│   ├── status-map.ts      # Default status mapping for this platform
│   ├── description.ts     # Description format conversion (if needed)
│   └── types.ts           # Platform-specific TypeScript types
```

### Status Map Type

```typescript
/**
 * Bidirectional mapping between platform statuses and HippoStatus.
 */
interface StatusMap {
  /** Platform status name → HippoStatus */
  toHippo: Record<string, HippoStatus>;

  /** HippoStatus → Platform status name */
  fromHippo: Record<HippoStatus, string>;

  /**
   * What to do when a platform status has no mapping.
   * "preserve" → use the platform value in status_raw, set status to fallback
   * "error" → throw an error
   */
  unmappedBehavior: "preserve" | "error";

  /** Fallback HippoStatus when unmappedBehavior is "preserve". Default: "todo". */
  fallbackStatus: HippoStatus;
}
```

---

## 4. MCP Server Design

### Overview

The MCP server is a standalone process that exposes HippoTask capabilities to AI agents. It manages its own task storage (in-memory or file-backed) and optionally connects to platform adapters.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     MCP Host (e.g. Claude)                  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    MCP Client                         │   │
│  └──────────────┬───────────────────────────────────────┘   │
│                  │ JSON-RPC 2.0 (stdio or HTTP)              │
└──────────────────┼──────────────────────────────────────────┘
                   │
┌──────────────────┼──────────────────────────────────────────┐
│  @hippotask/mcp-server                                      │
│                  │                                           │
│  ┌───────────────▼───────────────────────────────────────┐  │
│  │              MCP Server (SDK)                          │  │
│  │  ┌─────────┐  ┌───────────┐  ┌─────────────────────┐ │  │
│  │  │  Tools  │  │ Resources │  │      Prompts        │ │  │
│  │  └────┬────┘  └─────┬─────┘  └──────────┬──────────┘ │  │
│  └───────┼─────────────┼───────────────────┼────────────┘  │
│          │             │                   │                │
│  ┌───────▼─────────────▼───────────────────▼────────────┐  │
│  │                  Task Manager                         │  │
│  │  ┌──────────┐  ┌──────────────┐  ┌────────────────┐  │  │
│  │  │  Store   │  │   Validator  │  │  Adapter Pool  │  │  │
│  │  │(mem/file)│  │  (@core/zod) │  │  (optional)    │  │  │
│  │  └──────────┘  └──────────────┘  └───────┬────────┘  │  │
│  └──────────────────────────────────────────┼───────────┘  │
│                                              │              │
│  ┌──────────────────────────────────────────┼───────────┐  │
│  │               Connected Adapters          │           │  │
│  │  ┌──────┐  ┌────────┐  ┌───────┐  ┌─────▼───┐       │  │
│  │  │ Jira │  │ Linear │  │GitHub │  │  ...    │       │  │
│  │  └──────┘  └────────┘  └───────┘  └─────────┘       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Tools (8 tools)

| Tool Name | Input Schema | Output | Description |
|-----------|-------------|--------|-------------|
| `hippotask_create` | `{ title, description?, status?, priority?, assignees?, due_date?, labels?, project_id?, parent_id?, estimate?, custom_fields? }` | `HippoTask` | Create a new task |
| `hippotask_get` | `{ id }` | `HippoTask` | Get a task by its HippoTask ID |
| `hippotask_list` | `{ project_id?, status?, assignee?, labels?, priority?, search?, limit?, cursor?, sort_by?, sort_direction? }` | `PaginatedResult<HippoTask>` | List/search tasks |
| `hippotask_update` | `{ id, title?, description?, status?, priority?, assignees?, due_date?, labels?, estimate?, custom_fields? }` | `HippoTask` | Update a task |
| `hippotask_delete` | `{ id }` | `{ success: boolean }` | Delete a task |
| `hippotask_sync_pull` | `{ platform, project_id?, since? }` | `{ pulled: number, tasks: HippoTask[] }` | Pull tasks from a connected platform |
| `hippotask_sync_push` | `{ platform, task_ids?, project_id? }` | `{ pushed: number, results: SyncResult[] }` | Push tasks to a connected platform |
| `hippotask_connect` | `{ platform, config }` | `{ connected: boolean, capabilities: AdapterCapabilities }` | Connect a platform adapter |

### Resources (5 resources)

| URI Pattern | Description |
|-------------|-------------|
| `hippotask://tasks` | List of all tasks (compact) |
| `hippotask://tasks/{id}` | Full task detail |
| `hippotask://projects` | List of all projects |
| `hippotask://adapters` | Connected adapters and their status/capabilities |
| `hippotask://schema` | The HippoTask JSON Schema definition |

### Prompts (3 prompts)

| Prompt Name | Arguments | Description |
|-------------|-----------|-------------|
| `hippotask_plan_work` | `{ objective: string }` | Helps break down an objective into HippoTask tasks |
| `hippotask_status_report` | `{ project_id?: string }` | Generates a status summary of current tasks |
| `hippotask_triage` | `{ criteria?: string }` | Helps prioritize and organize pending tasks |

### Storage Interface

The MCP server needs local storage for tasks (since HippoTask is BYO-storage, but the MCP server needs something to work with). Two built-in options:

```typescript
/**
 * Minimal storage interface for the MCP server.
 * Users can implement their own (SQLite, Postgres, etc.)
 */
interface TaskStore {
  get(id: string): Promise<HippoTask | null>;
  list(query?: TaskQuery): Promise<PaginatedResult<HippoTask>>;
  create(task: HippoTask): Promise<HippoTask>;
  update(id: string, updates: Partial<HippoTask>): Promise<HippoTask>;
  delete(id: string): Promise<boolean>;
  clear(): Promise<void>;
}
```

**MemoryTaskStore** — In-memory Map. Perfect for AI agent sessions. Data is lost when the process exits.

**FileTaskStore** — Reads/writes to a JSON file. Good for persistent agent workflows or CLI usage. Simple but not concurrent-safe.

Users can provide their own `TaskStore` implementation for more advanced needs.

### MCP Server Configuration

```typescript
interface McpServerConfig {
  /** Server name shown to MCP clients. Default: "hippotask" */
  name?: string;

  /** Server version. Default: package version */
  version?: string;

  /** Storage backend. Default: MemoryTaskStore */
  store?: TaskStore;

  /** Pre-configured adapters to connect on startup. */
  adapters?: Array<{
    platform: string;
    config: unknown;
  }>;

  /** Transport configuration. */
  transport?: {
    type: "stdio" | "http";
    /** For HTTP transport: port number. Default: 3000 */
    port?: number;
  };
}
```

---

## 5. Sync Patterns

### Pattern 1: One-Way Pull (Platform → HippoTask)

The simplest sync pattern. Pull tasks from a platform into HippoTask's local store.

```
Jira ──── adapter.listTasks() ───→ HippoTask Store
          adapter.getChangedTasks(since) ───→ (incremental)
```

**Implementation:**
1. Call `adapter.listTasks()` for initial full sync.
2. Store results with `external_ids.{platform}` populated.
3. On subsequent syncs, call `adapter.getChangedTasks(lastSyncTimestamp)`.
4. Merge changes into existing tasks (match by `external_ids`).

**Conflict strategy:** Remote always wins (pull = source of truth is the platform).

### Pattern 2: One-Way Push (HippoTask → Platform)

Push locally-created tasks to a platform.

```
HippoTask Store ──── adapter.createTask() ───→ Linear
                     adapter.updateTask() ───→ (existing)
```

**Implementation:**
1. User creates/updates a task in HippoTask.
2. On push, check if `external_ids.{platform}` exists:
   - If yes → `adapter.updateTask(externalId, task)`
   - If no → `adapter.createTask(task)`, store returned external ID.

**Conflict strategy:** Local always wins (push = source of truth is HippoTask).

### Pattern 3: Bidirectional Sync (Complex)

Full two-way sync between HippoTask and one or more platforms.

```
Jira ←──── pull ────→ HippoTask Store ←──── push ────→ Linear
     ←── webhook ──→                   ←── webhook ──→
```

**Implementation:**
1. Each task tracks `external_ids` for all synced platforms.
2. Each task tracks `updated_at` for conflict detection.
3. On pull: if remote `updated_at` > local `updated_at`, update local.
4. On push: if local `updated_at` > last push timestamp, push to remote.
5. Conflict (both changed since last sync): apply conflict strategy.

**Conflict strategies:**
- `last_write_wins` — Most recent `updated_at` wins
- `local_wins` — HippoTask version always wins
- `remote_wins` — Platform version always wins
- `manual` — Flag as conflicted, surface to user/agent

**Note:** Bidirectional sync is a Phase 3+ feature. It requires careful handling of:
- Circular updates (push triggers webhook which triggers pull)
- Field-level merging (only some fields changed on each side)
- Timestamp precision differences between platforms

### Webhook Handling (Future)

For platforms that support webhooks, the adapter-common package provides a base webhook handler:

```typescript
interface WebhookHandler {
  /** Verify webhook signature/authenticity. */
  verify(request: WebhookRequest): boolean;

  /** Parse the webhook payload into a TaskChangeEvent. */
  parse(request: WebhookRequest): TaskChangeEvent;
}
```

The MCP server (in HTTP transport mode) can expose a webhook endpoint that dispatches to the appropriate adapter's handler.

---

## 6. Error Handling

### Error Types

```typescript
/** Base error for all HippoTask errors. */
class HippoError extends Error {
  readonly code: string;
  readonly context?: Record<string, unknown>;
}

/** Schema validation failed. */
class ValidationError extends HippoError {
  readonly code = "VALIDATION_ERROR";
  readonly issues: ZodIssue[];
}

/** Platform API call failed. */
class AdapterError extends HippoError {
  readonly code = "ADAPTER_ERROR";
  readonly platform: string;
  readonly statusCode?: number;
}

/** Rate limit exceeded. */
class RateLimitError extends AdapterError {
  readonly code = "RATE_LIMIT_ERROR";
  readonly retryAfter?: number; // seconds
}

/** Task not found on platform. */
class NotFoundError extends AdapterError {
  readonly code = "NOT_FOUND";
  readonly externalId: string;
}

/** Authentication failed. */
class AuthError extends AdapterError {
  readonly code = "AUTH_ERROR";
}

/** Sync conflict detected. */
class ConflictError extends HippoError {
  readonly code = "CONFLICT";
  readonly localTask: HippoTask;
  readonly remoteTask: HippoTask;
}
```

### Retry Strategy

Adapter-common provides a retry utility:

- **Transient errors** (5xx, timeout, network): retry with exponential backoff
- **Rate limit errors** (429): retry after `Retry-After` header or `retryAfter` value
- **Auth errors** (401, 403): do NOT retry — surface immediately
- **Not found** (404): do NOT retry
- **Validation errors**: do NOT retry — fix input

Default: 3 retries, 1s initial delay, 2x backoff, 30s max delay.

---

## 7. Testing Strategy

### Unit Tests (per package)

- **@hippotask/core**: Schema validation, ID generation, date helpers, merge logic
- **@hippotask/adapter-common**: Rate limiter, retry, cache, status map helpers
- **@hippotask/adapter-{platform}**: Field mapping logic (mock API responses → HippoTask and back)
- **@hippotask/mcp-server**: Tool handlers, resource handlers, store implementations

### Integration Tests (adapter-specific)

Each adapter has integration tests that run against a mock HTTP server (using `msw` or similar):

- Test full CRUD cycle: create → get → update → list → delete
- Test pagination handling
- Test error scenarios (rate limit, not found, auth failure)
- Test description format conversion round-trip
- Test status and priority mapping

### Contract Tests

Ensure all adapters conform to the `HippoAdapter` interface:

- Generic test suite that takes an adapter instance
- Runs the same tests against every adapter
- Validates that all adapters produce valid HippoTask objects

### MCP Integration Tests

Test the MCP server with a mock MCP client:

- Verify all tools accept valid input and produce valid output
- Verify resources return correct data
- Test adapter connection lifecycle
- Test sync flows (pull, push)

### Test Infrastructure

```
vitest.config.ts (root)
├── workspace: packages/*
├── coverage: v8
└── reporters: default + junit (for CI)
```

Each package has its own test directory:
```
packages/{package}/
├── src/
└── tests/
    ├── unit/
    └── integration/
```

---

## 8. Consumer Ease-of-Use Patterns

> Full detail in [ENGINEERING.md § Consumer DX](./ENGINEERING.md#5-consumer-dx--making-it-stupidly-easy).

### Guiding Principle: Progressive Disclosure

HippoTask is layered so consumers encounter complexity only when they need it:

```
┌───────────────────────────────────────────────────────────────┐
│  Layer 1: @hippotask/core                                      │
│  → createTask(), validate(), types, JSON Schema                │
│  → Works everywhere (Node, browser, Deno, Bun, edge)          │
│  → Zero config, zero external deps (just Zod)                 │
│  → This is what 60% of users need                              │
├───────────────────────────────────────────────────────────────┤
│  Layer 2: @hippotask/adapter-{platform}                        │
│  → Connect to real platforms with one function call             │
│  → adapter.connect({ token }) → adapter.listTasks()            │
│  → Smart defaults for status/priority mapping                  │
│  → This is what 30% of users add on top                        │
├───────────────────────────────────────────────────────────────┤
│  Layer 3: @hippotask/mcp-server                                │
│  → Full AI agent integration                                   │
│  → createServer() → server.start() (2 lines!)                 │
│  → Optional adapter connections for platform sync              │
│  → This is what 10% of users need (but growing fast)           │
└───────────────────────────────────────────────────────────────┘
```

### Builder Pattern for Complex Configuration

When users need to customize, we provide a fluent builder pattern:

```typescript
// Simple case — one line
const server = createServer();

// Complex case — still readable
const server = createServer({
  store: "file",
  storePath: "./tasks.json",
  adapters: [
    { platform: "jira", config: { domain: "...", token: "..." } },
    { platform: "linear", config: { apiKey: "..." } },
  ],
  transport: { type: "http", port: 8080 },
});
```

### Factory Functions Over Raw Constructors

```typescript
// What consumers see (clean, simple)
import { createTask, validate, createServer } from "@hippotask/...";

// What they DON'T see (implementation complexity)
// Internal: Zod parsing, UUIDv7 generation, timestamp injection,
// schema version stamping, deep merge logic, etc.
```

### Tree-Shakeable Exports

Every package uses named exports (no default exports). Modern bundlers can tree-shake unused code:

```typescript
// Consumer only pays for what they use
import { validate } from "@hippotask/core";
// → Only the validation code is bundled, not ID generation, merge utils, etc.
```

### Cross-Reference

- **Full DX standards and rules** → [ENGINEERING.md](./ENGINEERING.md)
- **Multi-environment support** → [DISTRIBUTION.md](./DISTRIBUTION.md)
- **Example code** → [EXAMPLES.md](./EXAMPLES.md)
