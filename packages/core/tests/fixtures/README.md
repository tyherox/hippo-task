# Test Fixtures

Canonical examples of HippoTask and HippoProject payloads used in tests and as reference material for adapter authors, schema reviewers, and AI agents exploring the repo.

## Layout

```
fixtures/
├── valid/
│   ├── minimal-task.json       Smallest task that passes HippoTaskSchema (required fields only)
│   ├── full-task.json          Task with every field populated (including custom_fields, metadata)
│   └── minimal-project.json    Smallest valid HippoProject
└── invalid/
    ├── empty-title.json        title: "" — fails "Title must be non-empty"
    ├── whitespace-title.json   title: "   " — fails "Title must not be whitespace-only"
    ├── bad-status.json         status: "banana" — not in HIPPO_STATUSES enum
    ├── dates-out-of-order.json start_date > due_date — fails .refine()
    └── missing-required.json   schema_version missing — fails
```

## Guarantees

Each fixture under `valid/` must pass `HippoTaskSchema.safeParse()` (or `HippoProjectSchema.safeParse()` for projects). Each fixture under `invalid/` must fail, with the failure mode matching the filename. A smoke test (`tests/unit/fixtures.test.ts`) enforces this so fixtures don't drift as the schema evolves.

## How to use

**Adding a new fixture:** place the JSON file under `valid/` or `invalid/` and add an entry to `fixtures.test.ts`. Keep fixtures minimal — only the fields needed to demonstrate the scenario. If you need a richer example, put it in `full-task.json`.

**Referencing fixtures in tests:** use `readFileSync` + `JSON.parse` or import via `await import("../fixtures/…", { assert: { type: "json" } })`. Do not construct equivalent objects inline — fixtures are the source of truth for "what a HippoTask looks like."

**Platform response fixtures** (`jira-issue.json`, `linear-issue.json`, etc.) belong in each adapter's own `tests/fixtures/` once adapters exist. This directory is scoped to core schema shapes only.
