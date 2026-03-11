# HippoTask — Engineering Standards

> SOLID principles, TDD strategy, code quality rules, and developer experience (DX) for consumers.
>
> This document is both a guide for HippoTask contributors AND a guarantee to HippoTask consumers about the quality they can expect.

---

## Table of Contents

- [1. Core Engineering Principles](#1-core-engineering-principles)
- [2. SOLID in Practice](#2-solid-in-practice)
- [3. Test-Driven Development (TDD)](#3-test-driven-development-tdd)
- [4. Code Quality Standards](#4-code-quality-standards)
- [5. Consumer DX — Making It Stupidly Easy](#5-consumer-dx--making-it-stupidly-easy)
- [6. API Design Rules](#6-api-design-rules)
- [7. Error Philosophy](#7-error-philosophy)
- [8. Documentation Standards](#8-documentation-standards)
- [9. Review Checklist](#9-review-checklist)

---

## 1. Core Engineering Principles

These are non-negotiable. Every PR, every design decision, every package must adhere:

### 1.1 Correctness Over Cleverness
Write boring, readable code. No clever tricks. If a reviewer can't understand it in 30 seconds, rewrite it.

### 1.2 Tests Prove Behavior
If there's no test, the behavior doesn't exist. Features ship with tests. Bug fixes ship with regression tests. No exceptions.

### 1.3 Small Surface Area
Every public export is a maintenance commitment. Export the minimum. Prefer a few well-designed functions over many options. Internal complexity is fine; public complexity is a bug.

### 1.4 Fail Loudly, Recover Gracefully
Validation errors are thrown immediately with clear messages. Network failures are retried with backoff. Never swallow errors. Never return `undefined` when you should throw.

### 1.5 Zero Surprises
A function called `createTask` should create a task. It shouldn't also log analytics, send emails, or modify global state. Side effects are explicit and documented.

---

## 2. SOLID in Practice

We don't just name-drop SOLID — here's how each principle manifests in HippoTask's architecture.

### S — Single Responsibility

| Package | Single Responsibility |
|---------|----------------------|
| `@hippotask/core` | Define and validate task shapes. Nothing else. |
| `@hippotask/adapter-common` | Shared adapter infrastructure (interfaces, rate limiting, retry). No platform-specific code. |
| `@hippotask/adapter-jira` | Jira ↔ HippoTask translation. No Asana logic. No sync orchestration. |
| `@hippotask/mcp-server` | Expose tasks to AI agents via MCP. No direct platform API calls (delegates to adapters). |

**Within packages:**

```typescript
// ✅ GOOD — each module has one job
// schema/task.ts    → defines the HippoTask Zod schema
// validation/rules.ts → business validation rules
// utils/id.ts       → ID generation

// ❌ BAD — god module that does everything
// task-utils.ts → defines schema AND validates AND generates IDs AND merges
```

**Rule:** If a file is doing two things, split it. If a class has two reasons to change, split it.

### O — Open/Closed

Adapters are the textbook example. The system is **open for extension** (add new adapters) but **closed for modification** (adding a Todoist adapter never touches `@hippotask/core` or the Jira adapter).

```typescript
// The adapter interface is the extension point.
// New adapters implement it without modifying existing code.

// ✅ Adding Todoist support:
// 1. Create @hippotask/adapter-todoist
// 2. Implement HippoAdapter<TodoistConfig>
// 3. Done. Nothing else changes.

// ❌ Wrong approach:
// Adding a `case "todoist":` to a switch statement in core.
```

**Extension points in HippoTask:**
- `HippoAdapter<T>` interface — extend with new platforms
- `TaskStore` interface — extend with new storage backends
- `StatusMap` / `PriorityMap` — extend with custom mappings
- `custom_fields` / `metadata` — extend task data without schema changes
- MCP tools/resources/prompts — extend server capabilities

### L — Liskov Substitution

Any `HippoAdapter` implementation must be interchangeable. Code that takes a `HippoAdapter` must work identically whether it receives `JiraAdapter`, `LinearAdapter`, or `GitHubAdapter`.

```typescript
// ✅ This function works with ANY adapter — it never inspects `adapter.platform`
async function pullAllTasks(adapter: HippoAdapter): Promise<HippoTask[]> {
  const result = await adapter.listTasks({ limit: 200 });
  return result.items;
}

// ❌ This violates LSP — it checks the concrete type
async function pullAllTasks(adapter: HippoAdapter): Promise<HippoTask[]> {
  if (adapter.platform === "jira") {
    // Special Jira handling — this means JiraAdapter isn't a true substitute
    return specialJiraFetch(adapter);
  }
  return (await adapter.listTasks()).items;
}
```

**Enforcement:** The contract test suite (shared across all adapters) guarantees substitutability. If an adapter passes the contract tests, it's a valid substitute.

### I — Interface Segregation

Adapters don't need to implement features they can't support. The `AdapterCapabilities` object declares what's available, and consumers check before calling.

```typescript
// ✅ GOOD — check capability before using
if (adapter.capabilities.webhooks && adapter.onTaskChange) {
  await adapter.onTaskChange(handleChange);
} else {
  // Fall back to polling
  const tasks = await adapter.getChangedTasks(lastSync);
}

// ✅ GOOD — optional methods are truly optional
interface HippoAdapter {
  // Required — every adapter must support these
  getTask(id: string): Promise<HippoTask>;
  listTasks(query?: TaskQuery): Promise<PaginatedResult<HippoTask>>;
  createTask(task: HippoTaskCreate): Promise<HippoTask>;

  // Optional — only if the platform supports it
  onTaskChange?(callback: (event: TaskChangeEvent) => void): Promise<() => void>;
}
```

**Future consideration:** If the adapter interface grows too large, split into role-specific interfaces:
```typescript
interface ReadableAdapter { getTask, listTasks }
interface WritableAdapter extends ReadableAdapter { createTask, updateTask, deleteTask }
interface SyncableAdapter extends WritableAdapter { getChangedTasks, onTaskChange }
```

### D — Dependency Inversion

High-level modules (MCP server, sync engine) depend on abstractions (interfaces), not concrete adapters.

```typescript
// ✅ GOOD — MCP server depends on the abstract HippoAdapter interface
class TaskManager {
  private adapters: Map<string, HippoAdapter> = new Map();

  registerAdapter(adapter: HippoAdapter): void {
    this.adapters.set(adapter.platform, adapter);
  }

  async syncPull(platform: string): Promise<HippoTask[]> {
    const adapter = this.adapters.get(platform);
    // Works with ANY adapter — no concrete imports
    return adapter.getChangedTasks(this.lastSync);
  }
}

// ✅ GOOD — Storage depends on an interface, not a concrete implementation
class McpServer {
  constructor(private store: TaskStore) {} // Could be Memory, File, SQLite, anything
}

// ❌ BAD — importing concrete implementations
import { JiraAdapter } from "@hippotask/adapter-jira";
class TaskManager {
  private jira = new JiraAdapter(); // Tight coupling!
}
```

---

## 3. Test-Driven Development (TDD)

### The TDD Workflow

We follow **strict Red-Green-Refactor** for all core logic:

```
1. RED    — Write a failing test that describes the desired behavior.
2. GREEN  — Write the minimum code to make the test pass.
3. REFACTOR — Clean up while keeping tests green.
```

### Test Categories & Requirements

| Category | Scope | Tool | Requirement |
|----------|-------|------|-------------|
| **Unit tests** | Single function / class | Vitest | Required for ALL public exports. ≥90% coverage on `@hippotask/core`. |
| **Contract tests** | Adapter conformance | Vitest + shared suite | Required for ALL adapters. Runs the same assertions against every adapter. |
| **Integration tests** | Adapter ↔ mock API | Vitest + `msw` | Required for ALL adapters. Full CRUD + error scenarios. |
| **MCP tests** | Tool/resource/prompt handlers | Vitest + MCP test client | Required for all MCP server handlers. |
| **Round-trip tests** | HippoTask → Platform → HippoTask | Vitest | Required for field mapping. Verify data survives the round trip. |
| **Snapshot tests** | JSON Schema output | Vitest snapshots | Catches unintended schema changes. |

### What We Test (and What We Don't)

**Always test:**
- Schema validation (valid inputs pass, invalid inputs fail with correct error messages)
- Field mapping (every core field correctly maps in both directions)
- Status/priority normalization (every mapping table entry)
- Edge cases (empty strings, null values, missing optional fields, Unicode, long strings)
- Error paths (what happens when the API returns 404? 429? 500? Timeout?)
- Pagination (multi-page results assembled correctly)
- ID generation (format correct, uniqueness)

**Never test:**
- Implementation details (private methods, internal state)
- Third-party library behavior (don't test that Zod works — test that *our* schema works)
- Trivial getters/setters with no logic

### Test File Convention

```
packages/{package}/
├── src/
│   ├── schema/
│   │   └── task.ts
│   └── utils/
│       └── id.ts
└── tests/
    ├── unit/
    │   ├── schema/
    │   │   └── task.test.ts        # Tests for task.ts
    │   └── utils/
    │       └── id.test.ts          # Tests for id.ts
    ├── integration/
    │   └── adapter.integration.test.ts
    └── fixtures/
        ├── valid-task.json         # Reusable test fixtures
        └── jira-issue-response.json
```

### Example: TDD for Schema Validation

```typescript
// Step 1: RED — write the test first

import { describe, it, expect } from "vitest";
import { HippoTaskSchema } from "../src/schema/task";

describe("HippoTaskSchema", () => {
  it("accepts a minimal valid task", () => {
    const result = HippoTaskSchema.safeParse({
      id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
      title: "Fix login bug",
      status: "todo",
      created_at: "2026-03-08T10:00:00Z",
      updated_at: "2026-03-08T10:00:00Z",
      schema_version: "1.0.0",
    });
    expect(result.success).toBe(true);
  });

  it("rejects empty title", () => {
    const result = HippoTaskSchema.safeParse({
      id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
      title: "",
      status: "todo",
      created_at: "2026-03-08T10:00:00Z",
      updated_at: "2026-03-08T10:00:00Z",
      schema_version: "1.0.0",
    });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0].path).toContain("title");
  });

  it("rejects invalid status", () => {
    const result = HippoTaskSchema.safeParse({
      id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
      title: "Fix login bug",
      status: "banana", // invalid
      created_at: "2026-03-08T10:00:00Z",
      updated_at: "2026-03-08T10:00:00Z",
      schema_version: "1.0.0",
    });
    expect(result.success).toBe(false);
  });

  it("validates start_date <= due_date", () => {
    const result = HippoTaskSchema.safeParse({
      id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
      title: "Task",
      status: "todo",
      start_date: "2026-03-15",
      due_date: "2026-03-10",  // before start_date!
      created_at: "2026-03-08T10:00:00Z",
      updated_at: "2026-03-08T10:00:00Z",
      schema_version: "1.0.0",
    });
    expect(result.success).toBe(false);
  });
});

// Step 2: GREEN — implement just enough to pass
// Step 3: REFACTOR — clean up while keeping tests green
```

### Contract Test Example

```typescript
// tests/contract/adapter-contract.test.ts
// This file is imported by every adapter's test suite.

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import type { HippoAdapter } from "@hippotask/adapter-common";
import { HippoTaskSchema } from "@hippotask/core";

export function runAdapterContractTests(
  name: string,
  createAdapter: () => Promise<HippoAdapter>
) {
  describe(`${name} — Adapter Contract`, () => {
    let adapter: HippoAdapter;

    beforeAll(async () => {
      adapter = await createAdapter();
    });

    afterAll(async () => {
      await adapter.disconnect();
    });

    it("has a non-empty platform identifier", () => {
      expect(adapter.platform).toBeTruthy();
      expect(typeof adapter.platform).toBe("string");
    });

    it("reports capabilities", () => {
      expect(adapter.capabilities).toBeDefined();
      expect(typeof adapter.capabilities.create).toBe("boolean");
      expect(typeof adapter.capabilities.webhooks).toBe("boolean");
    });

    it("creates a task and returns valid HippoTask", async () => {
      const created = await adapter.createTask({
        title: "Contract test task",
        status: "todo",
      });

      // Must be a valid HippoTask
      const validation = HippoTaskSchema.safeParse(created);
      expect(validation.success).toBe(true);

      // Must have external_ids populated
      expect(created.external_ids?.[adapter.platform]).toBeTruthy();
    });

    it("retrieves a created task", async () => {
      const created = await adapter.createTask({
        title: "Retrieve test",
        status: "todo",
      });
      const externalId = created.external_ids![adapter.platform]!;
      const retrieved = await adapter.getTask(externalId);

      expect(retrieved.title).toBe("Retrieve test");
      expect(HippoTaskSchema.safeParse(retrieved).success).toBe(true);
    });

    it("lists tasks with pagination", async () => {
      const result = await adapter.listTasks({ limit: 5 });

      expect(Array.isArray(result.items)).toBe(true);
      expect(typeof result.has_more).toBe("boolean");
      result.items.forEach((task) => {
        expect(HippoTaskSchema.safeParse(task).success).toBe(true);
      });
    });

    // ... more contract tests for update, delete, status mapping, etc.
  });
}
```

---

## 4. Code Quality Standards

### TypeScript Strictness

```jsonc
// tsconfig.base.json — strict everything
{
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "noPropertyAccessFromIndexSignature": true,
    "exactOptionalPropertyTypes": true,
    "forceConsistentCasingInFileNames": true,
    "isolatedModules": true,
    "verbatimModuleSyntax": true
  }
}
```

### Biome Rules

```jsonc
// biome.json
{
  "linter": {
    "rules": {
      "complexity": {
        "noExcessiveCognitiveComplexity": { "level": "error", "options": { "maxAllowedComplexity": 15 } }
      },
      "suspicious": {
        "noExplicitAny": "error",
        "noConfusingVoidType": "error"
      },
      "style": {
        "noNonNullAssertion": "warn",
        "useConst": "error"
      }
    }
  }
}
```

### Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Files | kebab-case | `status-map.ts`, `rate-limiter.ts` |
| Types / Interfaces | PascalCase | `HippoTask`, `AdapterCapabilities` |
| Functions / methods | camelCase | `createTask`, `getStatusMap` |
| Constants | SCREAMING_SNAKE | `DEFAULT_TIMEOUT`, `MAX_RETRIES` |
| Enum-like unions | snake_case strings | `"in_progress"`, `"multi_select"` |
| Package names | kebab-case | `@hippotask/adapter-jira` |

### Function Size Rule

Functions should be ≤ 30 lines (excluding comments). If longer, extract sub-functions. This is a guideline with rare exceptions (mapping functions may be longer).

### No `any`

`any` is banned. Use `unknown` for truly unknown data, then narrow with type guards or Zod.

```typescript
// ❌ BAD
function processMetadata(data: any) { ... }

// ✅ GOOD
function processMetadata(data: unknown): Record<string, unknown> {
  if (typeof data !== "object" || data === null) {
    throw new ValidationError("Metadata must be an object");
  }
  return data as Record<string, unknown>;
}

// ✅ BEST — use Zod
const MetadataSchema = z.record(z.unknown());
function processMetadata(data: unknown) {
  return MetadataSchema.parse(data);
}
```

---

## 5. Consumer DX — Making It Stupidly Easy

The #1 priority for HippoTask's public API is that a developer should be able to go from zero to working in under 5 minutes. Here's how we achieve that.

### 5.1 The 5-Minute Test

Every package must pass this test: a developer with no prior knowledge of HippoTask should be able to install the package, read the README, and have working code in under 5 minutes.

### 5.2 Minimal Boilerplate — Maximum Defaults

```typescript
// ✅ THIS IS WHAT WE'RE OPTIMIZING FOR:

// Use case 1: Validate a task (2 lines)
import { validate } from "@hippotask/core";
const result = validate({ title: "Fix bug", status: "todo" });

// Use case 2: Create a task with defaults (3 lines)
import { createTask } from "@hippotask/core";
const task = createTask({ title: "Fix bug" });
// → id: auto-generated, status: "todo", created_at: now, updated_at: now, schema_version: "1.0.0"

// Use case 3: Connect to Jira and pull tasks (5 lines)
import { JiraAdapter } from "@hippotask/adapter-jira";
const jira = new JiraAdapter();
await jira.connect({ domain: "myteam.atlassian.net", token: "..." });
const tasks = await jira.listTasks();

// Use case 4: Start MCP server for AI agents (3 lines)
import { createServer } from "@hippotask/mcp-server";
const server = createServer(); // sensible defaults: memory store, stdio transport
await server.start();
```

### 5.3 Smart Defaults, No Required Config

| Function | Default Behavior (zero config) |
|----------|-------------------------------|
| `createTask({ title })` | Auto-generates `id` (UUIDv7), sets `status: "todo"`, sets timestamps to now, sets `schema_version` |
| `new JiraAdapter()` | No config until `connect()` is called |
| `createServer()` | In-memory store, stdio transport, no adapters |
| `validate(task)` | Full validation with helpful error messages |

### 5.4 Helpful Error Messages

```typescript
// ❌ BAD error message
// Error: Validation failed

// ✅ GOOD error message
// HippoTaskValidationError: Invalid task:
//   • title: String must contain at least 1 character(s)
//   • status: Invalid enum value. Expected 'backlog' | 'todo' | 'in_progress' | 'in_review' | 'done' | 'cancelled', received 'banana'
//   • created_at: Invalid datetime string. Expected ISO 8601 format (e.g., "2026-03-08T10:00:00Z")
```

### 5.5 TypeScript Autocompletion Is Documentation

Design the types so that IDE autocompletion guides the developer:

```typescript
// When a developer types `status: "` their IDE should show:
// "backlog" | "todo" | "in_progress" | "in_review" | "done" | "cancelled"

// When a developer types `adapter.` their IDE should show:
// connect, disconnect, getTask, listTasks, createTask, updateTask, deleteTask,
// getProject, listProjects, getChangedTasks, capabilities, getStatusMap, setStatusMap
```

### 5.6 Progressive Complexity

```
Level 1 (Minute 1):   import { createTask } from "@hippotask/core"
Level 2 (Minute 3):   import { JiraAdapter } from "@hippotask/adapter-jira"
Level 3 (Minute 5):   import { createServer } from "@hippotask/mcp-server"
Level 4 (Hour 1):     Custom status maps, sync orchestration, multiple adapters
Level 5 (Day 1):      Custom adapter, custom storage backend, webhook handling
```

Each level adds capability without invalidating what came before.

### 5.7 Copy-Pasteable Examples

Every README, every doc page, every API reference includes examples that are:
- **Complete** — can be copy-pasted and run (no missing imports, no "..." elisions)
- **Minimal** — the shortest code that demonstrates the concept
- **Correct** — actually tested in CI (examples are test fixtures)

---

## 6. API Design Rules

### 6.1 Async by Default
All I/O operations return `Promise`. Even if the current implementation is synchronous (e.g., in-memory store), the interface is async so implementations can be swapped without breaking consumers.

### 6.2 Options Objects Over Positional Args
When a function takes more than 2 arguments, use an options object:

```typescript
// ❌ BAD — what is "true"? What is 50?
listTasks("project-123", "in_progress", true, 50);

// ✅ GOOD — self-documenting
listTasks({ project_id: "project-123", status: "in_progress", sort_direction: "asc", limit: 50 });
```

### 6.3 Result Types Over Thrown Errors (for Validation)
Validation returns a `Result` type. Consumers choose how to handle errors:

```typescript
// Result type
type Result<T, E> = { success: true; data: T } | { success: false; error: E };

// Usage — consumer decides
const result = validate(taskData);
if (result.success) {
  console.log(result.data); // typed as HippoTask
} else {
  console.error(result.error.issues); // typed as ValidationError
}
```

Adapter methods throw on I/O errors (network, auth, rate limit) because those are exceptional and non-recoverable by the caller without action.

### 6.4 No Side Effects in Constructors
Constructors allocate. `connect()` methods initialize. `disconnect()` methods clean up.

```typescript
// ✅ GOOD
const adapter = new JiraAdapter();     // just creates the object
await adapter.connect({ token: "..." }); // performs HTTP call
await adapter.disconnect();              // cleans up

// ❌ BAD
const adapter = new JiraAdapter({ token: "..." }); // constructor makes HTTP calls??
```

### 6.5 Immutable Public Data
HippoTask objects returned from adapters are treated as immutable snapshots. To update, call `updateTask()` with changes — don't mutate the returned object.

---

## 7. Error Philosophy

### Error Hierarchy

```
HippoError (base)
├── ValidationError      — bad input data (schema/business rule violation)
├── AdapterError         — platform API failure (base for adapter-specific)
│   ├── RateLimitError   — 429 / rate limit exceeded
│   ├── NotFoundError    — 404 / task doesn't exist
│   ├── AuthError        — 401/403 / bad credentials or permissions
│   └── PlatformError    — other API errors (500, malformed response, etc.)
└── ConflictError        — sync conflict between local and remote
```

### Error Principles

1. **Every error has a `code`** — machine-readable string for programmatic handling.
2. **Every error has a `message`** — human-readable string explaining what went wrong.
3. **Every adapter error includes `platform`** — know which platform failed.
4. **Rate limit errors include `retryAfter`** — consumers can wait and retry.
5. **Validation errors include `issues[]`** — every field that failed, with path and message.
6. **Errors are serializable** — `JSON.stringify(error)` produces useful output (for logging, MCP responses).

---

## 8. Documentation Standards

### Every Public Export Has JSDoc

```typescript
/**
 * Create a new HippoTask with sensible defaults.
 *
 * Generates a UUIDv7 `id`, sets `status` to "todo",
 * `created_at` and `updated_at` to now, and `schema_version` to current.
 *
 * @param input - Partial task data. Only `title` is required.
 * @returns A complete, validated HippoTask object.
 *
 * @example
 * ```typescript
 * const task = createTask({ title: "Fix login bug" });
 * console.log(task.id);     // "019470e1-..."
 * console.log(task.status); // "todo"
 * ```
 */
export function createTask(input: HippoTaskCreate): HippoTask { ... }
```

### Every Package Has a README

Package READMEs follow this structure:
1. One-line description
2. Install command
3. Quick start (≤10 lines of code)
4. API reference summary
5. Link to full documentation

### Changelog

Every package uses [Changesets](https://github.com/changesets/changesets) for versioning:
- Every PR that changes public behavior includes a changeset
- Changesets are aggregated into CHANGELOG.md on release
- Follow [Keep a Changelog](https://keepachangelog.com) format

---

## 9. Review Checklist

Every PR is reviewed against this checklist:

### Code Quality
- [ ] TypeScript strict mode passes with no `// @ts-ignore` or `as any`
- [ ] Biome lint passes with no suppressions
- [ ] Functions are ≤ 30 lines (with documented exceptions)
- [ ] No `console.log` (use proper error types or structured logging)
- [ ] Cognitive complexity ≤ 15 per function

### Testing
- [ ] New public functions have unit tests
- [ ] Bug fixes have regression tests
- [ ] Adapter changes pass contract test suite
- [ ] Edge cases are tested (empty input, null, boundary values)
- [ ] Error paths are tested (not just happy path)

### API Design
- [ ] Public API changes are backwards-compatible (or versioned with changeset)
- [ ] Options objects used for 3+ parameters
- [ ] Async interfaces for all I/O
- [ ] Errors are typed and include codes
- [ ] JSDoc on all public exports

### DX
- [ ] README updated if public API changed
- [ ] Examples are complete, copy-pasteable, and correct
- [ ] Error messages are helpful to someone who's never seen the codebase
- [ ] TypeScript autocompletion works well (no `string` where union types are possible)
