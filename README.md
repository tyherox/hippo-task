# 🦛 HippoTask

> **Hippocampus for modern work.**

An open schema and minimal toolkit for task interoperability — connecting custom apps, cross-platform workflows, and AI agents to the task management tools where real work happens.

---

## What is HippoTask?

HippoTask provides three things:

1. **A universal task schema** — A vendor-neutral representation of a task that captures the common denominator across Jira, Asana, Linear, ClickUp, Trello, GitHub, Notion, Monday.com, Todoist, and Microsoft Planner.

2. **Platform adapters** — Lightweight, bidirectional translators between the HippoTask schema and each platform's API.

3. **An MCP server** — First-class AI agent integration via the [Model Context Protocol](https://modelcontextprotocol.io), so agents can create, track, and sync tasks as part of their workflows.

HippoTask is **not** a UI, not a database, and not a Jira competitor. It's the connective tissue between tools.

## Quick Start

```typescript
// 1 line — create a task with smart defaults
import { createTask } from "@hippotask/core";
const task = createTask({ title: "Ship the login page" });

// 3 lines — connect to a platform
import { LinearAdapter } from "@hippotask/adapter-linear";
const linear = new LinearAdapter();
await linear.connect({ apiKey: "lin_api_..." });
const tasks = await linear.listTasks();

// 2 lines — start MCP server for AI agents
import { createServer } from "@hippotask/mcp-server";
await createServer().start();
```

## Use Cases

- **Building a custom task platform?** — Use HippoTask as your data model and get instant sync with 10+ platforms via adapters.
- **Working across multiple tools?** — Normalize tasks from all your tools into one canonical shape for dashboards, reports, or automation.
- **AI agent workflows?** — Give your agents structured task management via MCP, with a path to push results into your team's real tools.

## Provider Scorecard

How open and interoperable is each platform? We rate them on 8 dimensions.

| Provider | Overall | Best For |
|----------|:-------:|----------|
| **Linear** | ⭐ 4.5 | AI agents, modern DX |
| **GitHub** | ⭐ 4.4 | Open source, data portability |
| **Jira** | ⭐ 4.1 | Enterprise, custom fields |
| **ClickUp** | ⭐ 3.6 | Feature coverage |
| **Asana** | ⭐ 3.6 | Cross-functional teams |
| **Todoist** | ⭐ 3.1 | Personal tasks |
| **Monday.com** | ⭐ 3.1 | Dynamic schemas |
| **Trello** | ⭐ 2.9 | Simple Kanban |
| **Notion** | ⭐ 2.4 | Flexible properties |
| **MS Planner** | ⭐ 2.3 | Microsoft 365 ecosystem |

> See [Provider Scorecard](docs/PROVIDER_SCORECARD.md) for detailed ratings on all 8 dimensions.

## Documentation

### Vision & Strategy

| Document | Description |
|----------|-------------|
| [Vision](docs/VISION.md) | Project identity, use cases, non-goals, pros & cons, positioning |
| [Competitive Landscape](docs/COMPETITIVE_LANDSCAPE.md) | Deep analysis of competitors (OWL, TaskDef, Unito, Zapier, etc.) with pros/cons |
| [Provider Scorecard](docs/PROVIDER_SCORECARD.md) | Platform openness ratings on 8 dimensions with methodology |

### Technical Design

| Document | Description |
|----------|-------------|
| [Tech Specs](docs/TECH_SPECS.md) | Platform API landscape, field mapping matrix, technology decisions |
| [Schema](docs/SCHEMA.md) | Core schema specification with types, extensions, and format handling |
| [Architecture](docs/ARCHITECTURE.md) | Package structure, adapter interface, MCP server design, sync patterns |

### Engineering & Operations

| Document | Description |
|----------|-------------|
| [Engineering Standards](docs/ENGINEERING.md) | SOLID principles, TDD strategy, code quality, consumer DX |
| [Distribution](docs/DISTRIBUTION.md) | Packaging, publishing, multi-environment support, release process |
| [Examples & Playground](docs/EXAMPLES.md) | Self-contained demos, interactive playground prototype |
| [Roadmap](docs/ROADMAP.md) | 5-milestone delivery plan with acceptance criteria |

## Status

🚧 **Planning phase** — Schema design, architecture, and engineering standards complete. Implementation has not started.

## License

MIT
