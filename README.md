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

## Use Cases

- **Building a custom task platform?** — Use HippoTask as your data model and get instant sync with 10+ platforms via adapters.
- **Working across multiple tools?** — Normalize tasks from all your tools into one canonical shape for dashboards, reports, or automation.
- **AI agent workflows?** — Give your agents structured task management via MCP, with a path to push results into your team's real tools.

## Documentation

| Document | Description |
|----------|-------------|
| [Vision](docs/VISION.md) | Project identity, use cases, non-goals, and positioning |
| [Tech Specs](docs/TECH_SPECS.md) | Platform API landscape, field mapping matrix, technology decisions |
| [Schema](docs/SCHEMA.md) | Core schema specification with types, extensions, and format handling |
| [Architecture](docs/ARCHITECTURE.md) | Package structure, adapter interface, MCP server design, sync patterns |
| [Roadmap](docs/ROADMAP.md) | Phased delivery plan with milestones and acceptance criteria |

## Status

🚧 **Planning phase** — Schema design and architecture complete. Implementation has not started.

## License

MIT
