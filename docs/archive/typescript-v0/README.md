# HippoTask v0 — the TypeScript prototype (archived)

HippoTask's first implementation: a universal task-interop schema in TypeScript/Zod (`@hippotask/core`) plus adapter plumbing (`@hippotask/adapter-common`), with an MCP server and ten platform adapters planned. The code is preserved at git tag **`v0-typescript`**.

In September 2026 the core moved to Rust — `hippo-task` 0.1.0, at the repo root: a local, event-sourced task ledger for multi-agent coordination and audit. Why the scope narrowed and why Rust: `docs/DECISION.md` and `docs/decisions/adr-001-language.md`.

**Still useful:**
- `SCHEMA.md` — the 10-platform field study (Jira, Asana, Linear, ClickUp, Trello, GitHub, Notion, Monday.com, Todoist, Planner). The input for adapters, once sync is earned.
- `COMPETITIVE_LANDSCAPE.md`, `PROVIDER_SCORECARD.md`, `SCORECARD_RUBRIC.md`, `scorecard/` — market and provider research.
- `VISION.md` — the original "Hippocampus for modern work" vision. It predates the ledger: where it says HippoTask stores nothing, the Rust core supersedes it.

**History only:** `ARCHITECTURE.md`, `ENGINEERING.md`, `TECH_SPECS.md`, `ROADMAP.md`, `DISTRIBUTION.md`, `EXAMPLES.md`, and the prototype's own `PROJECT-README.md` and `AGENTS.md` describe the TypeScript plan.
