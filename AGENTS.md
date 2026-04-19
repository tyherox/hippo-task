# AGENTS.md — HippoTask

> Instructions for AI coding agents working on this repo.
>
> Claude Code reads this via `@AGENTS.md` in `CLAUDE.md`. Cursor, Aider, Codex, and other agent tools pick it up directly.

---

## Project identity

HippoTask is a TypeScript monorepo that provides (1) a universal task schema, (2) platform adapters for ~10 task-management tools (Jira, Linear, Asana, …), and (3) an MCP server so AI agents can manage tasks as part of their workflows. See [docs/VISION.md](docs/VISION.md) for the full "why."

**One-liner:** HippoTask is the connective tissue between task platforms, with first-class AI agent integration via MCP.

## Current status

**Planning / skeleton phase.** The schema and adapter interface are implemented in `@hippotask/core` and `@hippotask/adapter-common`. Concrete adapters, the MCP server, and published packages do not exist yet. The `docs/` directory is extensive and authoritative for design decisions — **read the relevant doc before implementing**. Don't invent APIs that contradict what's in docs; flag the gap and ask instead.

## Commands (run from repo root)

| Command | What it does |
|---|---|
| `pnpm install` | Install workspace dependencies (pnpm 10+ required) |
| `pnpm build` | Build all packages via turbo (tsup → `dist/`) |
| `pnpm test` | Run vitest in every package |
| `pnpm typecheck` | `tsc --noEmit` in every package |
| `pnpm lint` | Biome lint check |
| `pnpm lint:fix` | Biome lint + auto-fix |
| `pnpm schema:generate` | Regenerate `packages/core/schema/*.json` from Zod sources |
| `pnpm clean` | Remove all `dist/` |
| `pnpm --filter @hippotask/core <cmd>` | Scope any command to one package |

**Before reporting a task complete**, run `pnpm typecheck && pnpm test && pnpm lint`. All three must pass.

## Repo layout

```
hippo-task/
├── docs/                        Design docs — authoritative, read before implementing
├── packages/
│   ├── core/                    @hippotask/core — schema, validation, factories, utils
│   ├── adapter-common/          @hippotask/adapter-common — shared adapter infra (interface, retry, rate-limit, cache)
│   └── (planned) adapter-*/     Per-platform adapters (jira, linear, github, asana, …)
│   └── (planned) mcp-server/    MCP server for agent integration
├── .claude/                     Claude Code settings (permissions allowlist, future commands)
├── .hippotask/                  Local MCP task storage (gitignored — see "Storage convention")
├── AGENTS.md                    This file
├── CLAUDE.md                    Thin pointer to this file for Claude Code
└── README.md                    Public-facing overview
```

## Key concepts

- **HippoTask** — the canonical task shape ([`packages/core/src/schema/task.ts`](packages/core/src/schema/task.ts)). Required: `id`, `title`, `status`, `created_at`, `updated_at`, `schema_version`. Everything else optional. Zod schema with inferred TS type.
- **Statuses** (closed set): `backlog | todo | in_progress | in_review | done | cancelled`.
- **Priorities** (closed set): `none | low | medium | high | urgent`.
- **Adapter** — implements `HippoAdapter` from [`packages/adapter-common/src/adapter.ts`](packages/adapter-common/src/adapter.ts). Each declares `capabilities` honestly; consumers check before calling optional methods (Interface Segregation). Any adapter is substitutable for any other (Liskov) — **never** `if (adapter.platform === "jira")` in consumer code.
- **TaskStore** — storage interface for the MCP server ([`docs/ARCHITECTURE.md:443`](docs/ARCHITECTURE.md)). Built-in `MemoryTaskStore` (ephemeral) and `FileTaskStore` (JSON, atomic writes via write-to-temp + rename, not concurrent-safe). Users can BYO for SQLite/Postgres.
- **MCP server** — exposes task CRUD, sync, resources, and prompts to any MCP-compatible AI host. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §4.

## Storage convention

When the MCP server runs locally with `FileTaskStore`, it writes to `.hippotask/` at the repo root:

```
.hippotask/
├── tasks/{task-id}.json       One file per task — git-friendly diffs
├── projects/{project-id}.json
└── index.json                 Denormalized index for fast list queries
```

- **Gitignored by default.** Local agent scratch is not team-shared. Projects opt in to committing by removing the `.hippotask/` entry from `.gitignore`.
- **Single-writer.** `FileTaskStore` is not concurrent-safe. For parallel agents, use the (planned) SQLite store.
- **Remote.** Explicitly out of scope for v1. `TaskStore` is async, so a future `HttpTaskStore` drops in without breaking consumers. MCP's `transport: "http"` already covers "remote agent → local server."

The MCP server is not built yet, so `.hippotask/` does not exist until Milestone 3 ([docs/ROADMAP.md](docs/ROADMAP.md)) lands.

## Conventions

**TypeScript strictness** — see [`tsconfig.base.json`](tsconfig.base.json). Module: `Node16`, so **imports end in `.js`** even for `.ts` sources (`import { x } from "./foo.js"` next to `foo.ts`). No `any` — Biome rejects it. Prefer `unknown` + narrowing (ideally via Zod).

**Files** — kebab-case (`status-map.ts`). Types PascalCase. Functions camelCase. String-literal enums use snake_case (`"in_progress"`).

**Tests** — live in `packages/{pkg}/tests/`, not co-located. Mirror source tree: `src/utils/id.ts` → `tests/unit/utils/id.test.ts`. Vitest. Fixtures in `tests/fixtures/`.

**Public API** — only `packages/{pkg}/src/index.ts` exports ship. Adding an export is a maintenance commitment ([docs/ENGINEERING.md §1.3](docs/ENGINEERING.md)).

**Optional fields pattern** — the factory functions in [`packages/core/src/validation/factory.ts`](packages/core/src/validation/factory.ts) use conditional spreads to omit `undefined` rather than set it. Keep that pattern when adding optional fields.

**Commits** — short imperative titles. Follow what's in `git log`.

## Common agent tasks

### Add a field to the task schema

1. Edit [`packages/core/src/schema/task.ts`](packages/core/src/schema/task.ts) — add the Zod field.
2. If it's a closed set, add the enum to [`packages/core/src/schema/enums.ts`](packages/core/src/schema/enums.ts) and export it from `index.ts`.
3. If it has cross-field business rules, add a `.refine()` (see the `start_date <= due_date` precedent).
4. Add tests in [`packages/core/tests/unit/schema/task.test.ts`](packages/core/tests/unit/schema/task.test.ts) — valid input, invalid input, edge cases.
5. Run `pnpm schema:generate` to refresh `packages/core/schema/hippo-task.schema.json`.
6. Update [`docs/SCHEMA.md`](docs/SCHEMA.md).
7. **If the change is not a pure docstring tweak, bump the schema version** — see below.

### Bump the schema version

HippoTask versions its schema via a registry chain. Every parsed payload is validated against a `schema_version` enum; payloads at an older version can be upgraded via `migrateTask` / `migrateProject`.

To bump:

1. Pick major/minor/patch per [`packages/core/SCHEMA_CHANGELOG.md`](packages/core/SCHEMA_CHANGELOG.md). Patch-level edits (docstrings, refinements that don't reject previously-valid payloads) don't need a bump.
2. Append an entry to `REGISTRY` in [`packages/core/src/versioning/registry.ts`](packages/core/src/versioning/registry.ts) with a `migrateFromPrevious` pure function. `CURRENT_SCHEMA_VERSION` and `SUPPORTED_VERSIONS` update automatically.
3. Edit the Zod schemas to match the new shape.
4. Add tests in `packages/core/tests/unit/versioning/` — at minimum: migration happy path, idempotence for current-version input, clear failure for malformed input.
5. `pnpm --filter @hippotask/core schema:generate` — the output's `$id` and `title` pick up the new version.
6. Add an entry to [`SCHEMA_CHANGELOG.md`](packages/core/SCHEMA_CHANGELOG.md) with what changed, why, and migration notes.
7. Flag any adapter packages — they declare supported schema versions in their `capabilities`.

### Add a new platform adapter

1. Scaffold `packages/adapter-{platform}/` matching `packages/adapter-common/`'s layout (package.json, tsconfig.json, tsup.config.ts, vitest.config.ts, src/, tests/).
2. Implement `HippoAdapter<{Platform}Config>` from [`packages/adapter-common/src/adapter.ts`](packages/adapter-common/src/adapter.ts:64).
3. Declare `capabilities` honestly. If the platform can't do webhooks, set `webhooks: false` — don't fake it.
4. Status mapping in `src/status-map.ts` using `StatusMap` from adapter-common.
5. Pass the shared contract tests ([`docs/ENGINEERING.md §3`](docs/ENGINEERING.md)) — these prove substitutability.
6. Integration tests against a mocked API (`msw`), not the live platform.
7. Never make consumers `switch` on `adapter.platform` — LSP violation.

### Add an MCP tool / resource / prompt

Follow the layout in [`docs/ARCHITECTURE.md §4`](docs/ARCHITECTURE.md). One file per tool in `packages/mcp-server/src/tools/`. Zod-validate input. Return valid HippoTask objects. Test with an MCP test client.

## Gotchas

- **`.js` import extensions are required.** Node16 module resolution. New agents trip on this constantly.
- **Zod v3.** `z.string().datetime()` exists but isn't always used — the schema uses `z.string().min(1)` with docstring hints for ISO 8601 to keep error messages readable. Match the existing pattern.
- **`noNonNullAssertion` is a warning.** Avoid `!` anyway.
- **Cognitive complexity ≤ 15 per function**, enforced by Biome. Functions ≤ 30 lines is a guideline.
- **No module-level side effects.** `@hippotask/core` is `sideEffects: false`; don't introduce logging, registration, or global mutation at import time.
- **`exactOptionalPropertyTypes` is NOT enabled** — but the factory pattern (conditional spread) treats optional-or-absent as the invariant anyway. Keep doing that.

## Dependency between packages

`@hippotask/adapter-common` depends on `@hippotask/core` via `workspace:*`. Adapters will depend on both. The MCP server will depend on `@hippotask/core` only (adapters are loaded dynamically via `hippotask_connect`). Don't create back-edges — core must not import from adapter-common.

## Documentation map

Read when relevant; don't re-derive from code. These are authoritative for design decisions:

- [docs/VISION.md](docs/VISION.md) — project identity, three use cases, non-goals, positioning
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — package graph, adapter interface, MCP design, sync patterns, storage
- [docs/SCHEMA.md](docs/SCHEMA.md) — schema field-by-field, extensions, format handling
- [docs/TECH_SPECS.md](docs/TECH_SPECS.md) — per-platform API landscape, field mapping matrix
- [docs/ENGINEERING.md](docs/ENGINEERING.md) — SOLID, TDD, style, DX rules, review checklist
- [docs/DISTRIBUTION.md](docs/DISTRIBUTION.md) — packaging, publishing, multi-env (Deno/Bun/edge)
- [docs/EXAMPLES.md](docs/EXAMPLES.md) — self-contained demos + playground prototype
- [docs/ROADMAP.md](docs/ROADMAP.md) — 5-milestone plan with acceptance criteria
- [docs/PROVIDER_SCORECARD.md](docs/PROVIDER_SCORECARD.md) — per-platform openness ratings
- [docs/SCORECARD_RUBRIC.md](docs/SCORECARD_RUBRIC.md) — deterministic scoring rubric (authoritative source for the scorecard above)
- [docs/COMPETITIVE_LANDSCAPE.md](docs/COMPETITIVE_LANDSCAPE.md) — positioning vs. TaskDef, OWL, Unito

## Working style

- **Design first when docs are silent.** If `docs/` doesn't commit to something, flag the gap and ask. With 10 adapters to build, design drift is expensive.
- **Test-driven.** Public behavior without a test doesn't exist ([docs/ENGINEERING.md §1.2](docs/ENGINEERING.md)).
- **Small PRs.** One concern per PR.
- **Helpful errors.** Validation errors enumerate every failing field with path and reason ([docs/ENGINEERING.md §7](docs/ENGINEERING.md)). Don't ship "Validation failed."

## Dogfood plan

Once the MCP server ships (Milestone 3), this repo runs the server against itself. Agents working on HippoTask will track their own tasks in `.hippotask/` via MCP — the project tracks its own development. Items from [docs/ROADMAP.md](docs/ROADMAP.md) become real HippoTask tasks. This is both the sharpest possible demo and the tightest feedback loop for MCP server UX. Until then, `.hippotask/` is an empty convention.
