# HippoTask — Examples & Prototype

> Design spec for the examples section: self-contained demos that show HippoTask in action.
>
> These examples will eventually evolve into a dedicated interactive playground/service, but for now they're standalone, runnable projects.

---

## Table of Contents

- [1. Philosophy](#1-philosophy)
- [2. Example Matrix](#2-example-matrix)
- [3. Example 1: Quick Start](#3-example-1-quick-start)
- [4. Example 2: Multi-Platform Aggregator](#4-example-2-multi-platform-aggregator)
- [5. Example 3: MCP Agent Workflow](#5-example-3-mcp-agent-workflow)
- [6. Example 4: Interactive Playground (Prototype)](#6-example-4-interactive-playground-prototype)
- [7. Example Standards](#7-example-standards)
- [8. Future: Dedicated Service](#8-future-dedicated-service)

---

## 1. Philosophy

### Examples Are the First Thing People See

Most developers evaluate a library by looking at examples *before* reading docs. Our examples must be:

1. **Runnable in under 60 seconds** — clone, install, run. No external accounts required for basic examples.
2. **Self-contained** — each example is a standalone project with its own `package.json`. No monorepo knowledge needed.
3. **Progressive** — start simple, get more complex. Example 1 is 10 lines. Example 4 is a full prototype app.
4. **Realistic** — use real-world scenarios, not contrived `foo/bar` examples.
5. **Tested in CI** — examples are built and run in CI. They don't rot.

### Examples Double as Integration Tests

Every example is also a smoke test for the packages it uses. If an example breaks, we know before our users do.

---

## 2. Example Matrix

| # | Example | Packages Used | Complexity | External Accounts? |
|---|---------|--------------|------------|-------------------|
| 1 | **Quick Start** | `@hippotask/core` | Beginner | No |
| 2 | **Multi-Platform Aggregator** | `core` + `adapter-jira` + `adapter-linear` + `adapter-github` | Intermediate | Yes (tokens) |
| 3 | **MCP Agent Workflow** | `core` + `mcp-server` + `adapter-github` | Intermediate | Optional |
| 4 | **Interactive Playground** | `core` + `mcp-server` + all adapters | Advanced | Optional |

---

## 3. Example 1: Quick Start

> **Goal:** Show HippoTask's core schema in action — create, validate, transform tasks — in 10 lines of code. No external services.

### Directory Structure

```
examples/quick-start/
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts
└── README.md
```

### What It Demonstrates

```typescript
// examples/quick-start/src/index.ts

import { createTask, validate, HippoTaskSchema } from "@hippotask/core";

// 1. Create a task with smart defaults
const task = createTask({
  title: "Ship the login page",
  description: "## Requirements\n- OAuth 2.0\n- Google + GitHub providers",
  priority: "high",
  labels: ["frontend", "auth"],
  due_date: "2026-03-15",
});

console.log("Created task:", task);
// → { id: "019...", title: "Ship the login page", status: "todo", ... }

// 2. Validate arbitrary data
const result = validate({
  title: "",           // ← invalid: empty
  status: "banana",    // ← invalid: not a valid status
});

if (!result.success) {
  console.log("Validation errors:");
  result.error.issues.forEach((issue) => {
    console.log(`  • ${issue.path.join(".")}: ${issue.message}`);
  });
}

// 3. Use the Zod schema directly for advanced use cases
const parsed = HippoTaskSchema.safeParse(task);
console.log("Schema valid:", parsed.success);

// 4. Access the JSON Schema for other languages
// → Available at @hippotask/core/schema/hippo-task.schema.json
```

### README Content

```markdown
# HippoTask Quick Start

Create and validate tasks in 10 lines of code.

## Run it

npm install
npx tsx src/index.ts

## What you'll see

- A task created with auto-generated ID, timestamps, and defaults
- Validation errors for invalid input with helpful messages
- Schema validation using Zod

## Next Steps

- [Multi-Platform Aggregator](../multi-platform/) — connect to real platforms
- [MCP Agent Workflow](../mcp-agent/) — use with AI agents
```

---

## 4. Example 2: Multi-Platform Aggregator

> **Goal:** Pull tasks from 2-3 platforms into a unified HippoTask list. Show the normalization power.

### Directory Structure

```
examples/multi-platform/
├── package.json
├── tsconfig.json
├── .env.example          # Template for platform tokens
├── src/
│   ├── index.ts          # Main aggregation script
│   ├── mock-mode.ts      # Runs with mock data (no tokens needed)
│   └── report.ts         # Pretty-prints the aggregated results
└── README.md
```

### What It Demonstrates

```typescript
// examples/multi-platform/src/index.ts (simplified)

import { LinearAdapter } from "@hippotask/adapter-linear";
import { GitHubAdapter } from "@hippotask/adapter-github";
import type { HippoTask } from "@hippotask/core";
import { printReport } from "./report";

async function main() {
  const tasks: HippoTask[] = [];

  // Connect to Linear
  const linear = new LinearAdapter();
  await linear.connect({ apiKey: process.env.LINEAR_API_KEY! });
  const linearTasks = await linear.listTasks({ status: ["todo", "in_progress"] });
  tasks.push(...linearTasks.items);

  // Connect to GitHub
  const github = new GitHubAdapter();
  await github.connect({
    token: process.env.GITHUB_TOKEN!,
    owner: "tyherox",
    repo: "hippo-task",
  });
  const githubTasks = await github.listTasks({ status: "todo" });
  tasks.push(...githubTasks.items);

  // All tasks are now in the same HippoTask format!
  // Sort by due date across both platforms
  tasks.sort((a, b) => {
    if (!a.due_date) return 1;
    if (!b.due_date) return -1;
    return a.due_date.localeCompare(b.due_date);
  });

  printReport(tasks);

  await linear.disconnect();
  await github.disconnect();
}

main();
```

### Mock Mode

For users who don't have platform tokens, a mock mode uses fixture data:

```typescript
// examples/multi-platform/src/mock-mode.ts

import type { HippoTask } from "@hippotask/core";
import { createTask } from "@hippotask/core";
import { printReport } from "./report";

// Simulated tasks from different platforms
const tasks: HippoTask[] = [
  createTask({
    title: "Implement OAuth flow",
    status: "in_progress",
    priority: "high",
    labels: ["backend"],
    due_date: "2026-03-12",
    external_ids: { linear: "ENG-142" },
  }),
  createTask({
    title: "Fix CI pipeline timeout",
    status: "todo",
    priority: "urgent",
    labels: ["devops"],
    due_date: "2026-03-10",
    external_ids: { github: "tyherox/hippo-task#15" },
  }),
  createTask({
    title: "Design system color tokens",
    status: "todo",
    priority: "medium",
    labels: ["design", "frontend"],
    due_date: "2026-03-20",
    external_ids: { linear: "DES-89" },
  }),
];

printReport(tasks);
```

### Report Output

```
╔══════════════════════════════════════════════════════════════╗
║                  HippoTask Unified Report                   ║
║                  3 tasks from 2 platforms                    ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║  🔴 URGENT  Fix CI pipeline timeout                          ║
║     GitHub #15 · todo · due Mar 10 · [devops]                ║
║                                                              ║
║  🟠 HIGH    Implement OAuth flow                             ║
║     Linear ENG-142 · in_progress · due Mar 12 · [backend]    ║
║                                                              ║
║  🟡 MEDIUM  Design system color tokens                       ║
║     Linear DES-89 · todo · due Mar 20 · [design, frontend]   ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
```

---

## 5. Example 3: MCP Agent Workflow

> **Goal:** Show the MCP server in action — an AI agent managing tasks. Self-contained with in-memory storage.

### Directory Structure

```
examples/mcp-agent/
├── package.json
├── tsconfig.json
├── src/
│   ├── server.ts            # Starts the MCP server
│   ├── simulate-agent.ts    # Simulates an agent conversation
│   └── claude-config.json   # Claude Desktop config template
├── README.md
└── tasks/                   # File store location
    └── .gitkeep
```

### What It Demonstrates

1. **Starting the MCP server** with file-backed storage
2. **Agent tool calls** — creating, listing, updating tasks
3. **Optional platform sync** — pushing completed tasks to GitHub

```typescript
// examples/mcp-agent/src/server.ts

import { createServer } from "@hippotask/mcp-server";

const server = createServer({
  name: "hippotask-example",
  store: "file",
  storePath: "./tasks/agent-tasks.json",
  transport: { type: "stdio" },
});

console.error("HippoTask MCP server starting on stdio...");
await server.start();
```

```typescript
// examples/mcp-agent/src/simulate-agent.ts
// Simulates what an AI agent would do via MCP tool calls

import { createServer } from "@hippotask/mcp-server";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

async function simulateAgent() {
  // In reality, the AI host (Claude, etc.) would do this.
  // Here we simulate the tool calls programmatically.

  console.log("🤖 Agent: I need to break down the authentication feature.\n");

  // 1. Create parent task
  const parent = await callTool("hippotask_create", {
    title: "Implement user authentication",
    description: "Full OAuth 2.0 flow with Google and GitHub providers",
    priority: "high",
    labels: ["feature", "auth"],
  });
  console.log(`✅ Created: ${parent.title} (${parent.id})\n`);

  // 2. Create subtasks
  const subtasks = [
    { title: "Set up OAuth credentials", priority: "high" },
    { title: "Build login page UI", priority: "medium" },
    { title: "Implement callback handler", priority: "high" },
    { title: "Add session management", priority: "medium" },
    { title: "Write integration tests", priority: "medium" },
  ];

  for (const sub of subtasks) {
    const task = await callTool("hippotask_create", {
      ...sub,
      parent_id: parent.id,
      labels: ["auth"],
    });
    console.log(`  📋 Subtask: ${task.title}`);
  }

  // 3. List all tasks
  console.log("\n🤖 Agent: Let me check my task list.\n");
  const list = await callTool("hippotask_list", {
    sort_by: "priority",
    sort_direction: "desc",
  });
  console.log(`  Found ${list.items.length} tasks\n`);

  // 4. Complete a task
  console.log("🤖 Agent: I've set up the OAuth credentials.\n");
  await callTool("hippotask_update", {
    id: list.items[1].id,  // "Set up OAuth credentials"
    status: "done",
  });
  console.log("  ✅ Marked as done\n");

  // 5. Generate status report (via prompt)
  console.log("🤖 Agent: Here's my status report:\n");
  const report = await callPrompt("hippotask_status_report", {});
  console.log(report);
}

// Helper to simulate MCP tool calls (actual implementation would use MCP client)
async function callTool(name: string, args: Record<string, unknown>) {
  // Placeholder — in the real example, this connects to the MCP server
  console.log(`  [MCP] ${name}(${JSON.stringify(args).slice(0, 60)}...)`);
  return args;  // Simplified
}

async function callPrompt(name: string, args: Record<string, unknown>) {
  return "  5 tasks remaining. 1 completed. 2 high priority items due this week.";
}

simulateAgent();
```

### Claude Desktop Setup

```json
// examples/mcp-agent/src/claude-config.json
// Copy this to your Claude Desktop MCP config
{
  "mcpServers": {
    "hippotask": {
      "command": "npx",
      "args": ["tsx", "src/server.ts"],
      "cwd": "/path/to/examples/mcp-agent"
    }
  }
}
```

---

## 6. Example 4: Interactive Playground (Prototype)

> **Goal:** A self-contained web UI that demonstrates HippoTask's full capabilities. This is the prototype that will eventually become a dedicated service.

### Directory Structure

```
examples/playground/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   ├── main.ts              # App entry point
│   ├── App.tsx               # Main app component
│   ├── components/
│   │   ├── TaskEditor.tsx    # Create/edit a HippoTask with live validation
│   │   ├── TaskList.tsx      # Display tasks with status/priority badges
│   │   ├── SchemaViewer.tsx  # Interactive JSON Schema explorer
│   │   ├── AdapterDemo.tsx   # Mock adapter demonstration
│   │   ├── ScoreCard.tsx     # Interactive provider scorecard charts
│   │   └── FormatConverter.tsx # Live Markdown ↔ ADF/HTML/Blocks preview
│   ├── stores/
│   │   └── tasks.ts          # In-memory task store (using @hippotask/core)
│   └── styles/
│       └── main.css
├── public/
│   └── hippo-logo.svg
└── README.md
```

### Features

| Feature | What It Shows |
|---------|--------------|
| **Task Editor** | Form to create HippoTask objects with live Zod validation. Red/green indicators on each field. Generates the JSON output in real-time. |
| **Task List** | Kanban-style board showing tasks grouped by status. Drag-and-drop between columns. Priority badges. Platform source indicators. |
| **Schema Explorer** | Interactive view of the HippoTask JSON Schema. Click any field to see its type, constraints, and which platforms support it. |
| **Adapter Simulator** | Mock adapter that shows the transformation: `Platform JSON → HippoTask JSON → Platform JSON`. Side-by-side comparison. |
| **Provider Scorecard** | Interactive radar charts (Chart.js) for each provider. Filter/compare. Shows the data from PROVIDER_SCORECARD.md. |
| **Format Converter** | Live preview of description format conversion. Type Markdown, see ADF/HTML/Notion Blocks output in real-time. |

### Tech Stack (Playground Only)

| Concern | Choice | Reason |
|---------|--------|--------|
| Framework | React (or Preact) | Familiar, small, fast |
| Build | Vite | Fast dev server, good TS support |
| Styling | Tailwind CSS | Utility-first, no design system needed |
| Charts | Chart.js (via react-chartjs-2) | Radar charts for scorecard |
| State | Zustand or React state | Lightweight, no Redux overhead |

### Key Constraint: Self-Contained

The playground runs entirely in the browser. No backend server. No database. All state is in-memory (with optional localStorage persistence). This means:

- `@hippotask/core` runs natively in the browser ✅
- Adapter demos use **mock data** (no real API calls from the browser)
- MCP is shown conceptually (not a real MCP connection from browser)

### Running the Playground

```bash
cd examples/playground
npm install
npm run dev
# → Opens http://localhost:5173 with the playground
```

---

## 7. Example Standards

### Every Example Must:

- [ ] Have its own `package.json` (standalone install)
- [ ] Have a `README.md` with: what it does, how to run it, what you'll see
- [ ] Run with `npm install && npm start` (or `npm run dev` for the playground)
- [ ] Work without external platform accounts (mock mode available)
- [ ] Be tested in CI (build + run + basic output check)
- [ ] Use the latest published `@hippotask/*` packages (not workspace links in CI)

### CI Example Testing

```yaml
# .github/workflows/examples.yml (conceptual)
jobs:
  test-examples:
    strategy:
      matrix:
        example: [quick-start, multi-platform, mcp-agent, playground]
    steps:
      - run: cd examples/${{ matrix.example }} && npm install && npm run build
      - run: cd examples/${{ matrix.example }} && npm test  # if tests exist
      - run: cd examples/${{ matrix.example }} && npm start  # smoke test
```

---

## 8. Future: Dedicated Service

The playground prototype is the seed for a future dedicated service. Here's the evolution path:

### Phase 1: Static Playground (Current Plan)
- Browser-only, self-contained
- Mock adapters, in-memory storage
- Shows the schema, validation, and normalization concepts
- Ships as part of the monorepo under `examples/playground/`

### Phase 2: Connected Playground
- Add a lightweight backend (Node.js) that proxies real adapter calls
- User provides their own API tokens (stored in browser only)
- Real data flows through mock adapters → HippoTask → display
- Still runs locally (`npm run dev`)

### Phase 3: Hosted Service (Future)
- Deploy as a web app (Vercel, Cloudflare Pages + Workers, etc.)
- GitHub OAuth for authentication
- Persistent task store (Turso/SQLite, or Cloudflare D1)
- Live adapter connections
- Shareable workspace URLs
- This becomes the "try HippoTask without installing anything" experience

### Phase 4: HippoTask Hub (Long-term Vision)
- A place for teams to:
  - Configure their adapter connections
  - View unified task dashboards
  - Set up sync rules
  - Monitor adapter health
- Still open source. Self-hostable. But also available as a managed service.
- Revenue model: free for open source / personal use, paid for team features

**Important:** Phases 3-4 are aspirational. The current planning scope only covers Phase 1 (playground prototype as a self-contained example). We don't build a service until the library is solid.
