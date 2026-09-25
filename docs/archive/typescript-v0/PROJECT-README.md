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

## Provider Scorecard — 🟡 Preview

How open and interoperable is each platform? Every cell is scored **deterministically** against the [Scorecard Rubric](docs/SCORECARD_RUBRIC.md) — one YAML per provider at [`docs/scorecard/providers/`](docs/scorecard/providers), a CI-gated generator produces the numbers below and [`docs/scorecard.json`](docs/scorecard.json).

<!-- BEGIN:AUTOGEN_README_SCORECARD -->
| Provider | Overall | Best For |
|----------|:-------:|----------|
| **Jira** | ⭐ 4.5 | Enterprise, custom fields |
| **GitHub** | ⭐ 4.4 | Open source, data portability |
| **Linear** | ⭐ 4.3 | AI agents, modern DX |
| **Monday.com** | ⭐ 4.3 | Dynamic schemas |
| **Asana** | ⭐ 4.0 | Cross-functional teams |
| **ClickUp** | ⭐ 3.9 | Feature coverage |
| **Todoist** | ⭐ 3.9 | Personal tasks |
| **Notion** | ⭐ 3.6 | Flexible properties |
| **Trello** | ⭐ 3.3 | Simple Kanban |
| **MS Planner** | ⭐ 3.0 | Microsoft 365 ecosystem |
<!-- END:AUTOGEN_README_SCORECARD -->

> **Preview status.** The methodology is still settling — we're publishing now to gather external feedback. If you think a cell is wrong, [open an issue](https://github.com/hippotask/hippo-task/issues/new) with a vendor-doc URL showing why; maintainers re-apply the rubric and ship a patch audit if the cell moves. See the full §3 detailed ratings, §5 gotcha summary, and audit history in [docs/PROVIDER_SCORECARD.md](docs/PROVIDER_SCORECARD.md).

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
