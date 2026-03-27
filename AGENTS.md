# HippoTask — Root AGENTS.md

> This file is for AI agents working on this codebase. Read it first before making any changes.

## What Is This Project?

HippoTask is an **open-source task interoperability toolkit**. It provides:
- A universal task schema (Zod-based, JSON Schema exported)
- Platform adapter interfaces for syncing tasks to/from Jira, Linear, GitHub, etc.
- A CLI tool designed for both humans and AI agents, with multi-agent safety features

It is a **library and toolset**, NOT a web app, NOT a database, NOT a Jira competitor.

## Monorepo Layout

```
hippo-task/
├── .cursorrules              ← LLM generation rules (READ THIS)
├── AGENTS.md                 ← YOU ARE HERE — high-level guide
├── AI_README.md              ← Guide for AI agents USING the CLI tool
├── packages/
│   ├── core/                 ← @hippotask/core — schema, validation, utilities
│   │   └── AGENTS.md         ← Domain guide for core package
│   ├── adapter-common/       ← @hippotask/adapter-common — interfaces, shared utils
│   │   └── AGENTS.md         ← Domain guide for adapter package
│   └── cli/                  ← @hippotask/cli — CLI with safety layers
│       └── AGENTS.md         ← Domain guide for CLI package
├── docs/                     ← Planning & design documents
│   └── AGENTS.md             ← Documentation standards
├── biome.json                ← Linting (Biome)
├── tsconfig.base.json        ← Strict TypeScript config
├── turbo.json                ← Build pipeline (Turborepo)
└── pnpm-workspace.yaml       ← Monorepo config
```

## Architecture Overview

```
┌───────────────────────────────────────────────────────┐
│                   @hippotask/core                      │
│  Zod schemas │ Types │ Validation │ Factory │ Utils    │
│  Zero dependencies (except Zod)                        │
│  Works everywhere: Node, Deno, Bun, browsers, edge     │
└────────────────────┬──────────────────────────────────┘
                     │ depends on
┌────────────────────▼──────────────────────────────────┐
│              @hippotask/adapter-common                  │
│  HippoAdapter<T> interface │ StatusMap │ PriorityMap    │
│  RateLimiter │ withRetry │ TtlCache │ Error types      │
└────────────────────┬──────────────────────────────────┘
                     │ depends on
┌────────────────────▼──────────────────────────────────┐
│                  @hippotask/cli                         │
│  11 commands │ FileTaskStore │ 4 safety layers          │
│  File locking │ Optimistic locking │ Claiming │ Audit   │
└───────────────────────────────────────────────────────┘
```

**Dependency flow is strictly top-down.** Core never imports from adapter-common or CLI. Adapter-common never imports from CLI. Violations will break the build.

## How to Work in This Codebase

### Step 0: Read the relevant AGENTS.md
Every package has its own `AGENTS.md` with domain-specific guidance. Read the one for the package you're modifying BEFORE writing code.

### Step 1: Write tests FIRST (TDD)
```bash
# Create your test file
# packages/{pkg}/tests/unit/{module}.test.ts

# Run tests (they should FAIL — RED phase)
pnpm test --filter @hippotask/{pkg}
```

### Step 2: Implement to make tests pass (GREEN)
```bash
# Write minimal code in packages/{pkg}/src/{module}.ts
# Run tests again (they should PASS)
pnpm test --filter @hippotask/{pkg}
```

### Step 3: Refactor if needed
Clean up while keeping tests green.

### Step 4: Verify everything
```bash
pnpm build    # All packages build
pnpm test     # All 272+ tests pass
pnpm lint     # Biome passes
```

### Step 5: Update AGENTS.md if needed
If you added a new module, command, gotcha, or changed architecture, update the relevant `AGENTS.md`.

### Step 6: Commit
```bash
git add -A && git commit -m "Imperative description of WHAT and WHY"
```

## Key Technical Decisions

| Decision | Rationale | Reference |
|----------|-----------|-----------|
| TypeScript strict mode | Catch bugs at compile time | `tsconfig.base.json` |
| Zod for schemas | Runtime validation + type inference + JSON Schema export | `packages/core/src/schema/` |
| Vitest for testing | Fast, TS-native, good DX | `vitest.config.ts` in each package |
| tsup for builds | ESM + CJS dual output, fast | `tsup.config.ts` in each package |
| Biome for linting | Fast, replaces ESLint + Prettier | `biome.json` |
| pnpm workspaces | Efficient monorepo, strict deps | `pnpm-workspace.yaml` |
| Turborepo | Cached builds, parallel tasks | `turbo.json` |
| UUIDv7 for IDs | Time-sortable, globally unique | `packages/core/src/utils/id.ts` |
| Markdown for descriptions | 6/10 platforms native, converters for rest | `docs/SCHEMA.md §7` |
| metadata for safety | Claims + activity log in metadata, no schema changes | `packages/cli/src/safety.ts` |

## Common Commands

```bash
pnpm install              # Install all dependencies
pnpm build                # Build all packages (via Turborepo)
pnpm test                 # Run all tests (via Turborepo)
pnpm lint                 # Lint all code (Biome)
pnpm lint:fix             # Auto-fix lint issues

# Package-specific
pnpm test --filter @hippotask/core
pnpm build --filter @hippotask/cli

# CLI tool (after build)
node packages/cli/dist/bin.js --help
node packages/cli/dist/bin.js create "Test task" --store /tmp/test.json
```

## Test Counts (keep this updated)

| Package | Tests | Coverage Target |
|---------|-------|----------------|
| `@hippotask/core` | 156 | ≥90% |
| `@hippotask/adapter-common` | 38 | ≥80% |
| `@hippotask/cli` | 78 | ≥80% |
| **Total** | **272** | — |

## Don't Forget

- **Read `.cursorrules`** — it has ALL the code style and quality rules.
- **Read the package's `AGENTS.md`** before modifying that package.
- **Tests first** — if there's no failing test, don't write implementation code.
- **Update `AGENTS.md`** when you add modules, discover gotchas, or change patterns.
- **Never break the dependency graph** — core → adapter-common → cli (one direction only).
