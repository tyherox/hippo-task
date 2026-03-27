# @hippotask/core — Domain Guide

> Read this before modifying anything in `packages/core/`.

## What This Package Does

Defines the **universal task schema** and provides validation, factory functions, and utilities. This is the foundation — every other package depends on it.

**Key principle:** This package has ZERO knowledge of adapters, CLI, or any specific platform. It defines shapes, not behavior.

## Module Map

```
src/
├── schema/                    ← Zod schemas (the heart of the project)
│   ├── enums.ts               ← HippoStatus, HippoPriority, etc.
│   ├── task.ts                ← HippoTask schema (24 fields)
│   ├── project.ts             ← HippoProject schema (9 fields)
│   ├── person.ts              ← HippoPerson (lightweight user ref)
│   ├── custom-field.ts        ← HippoCustomField (typed extension)
│   ├── query.ts               ← TaskQuery + PaginatedResult
│   └── events.ts              ← TaskChangeEvent (for sync)
├── validation/
│   ├── validate.ts            ← validateTask(), validateProject()
│   └── factory.ts             ← createTask(), createProject() with smart defaults
├── utils/
│   ├── id.ts                  ← generateId() — UUIDv7
│   ├── dates.ts               ← nowISO(), isValidISO8601(), isDateOnOrBefore()
│   └── merge.ts               ← deepMerge() for partial updates
├── errors.ts                  ← HippoError, ValidationError
└── index.ts                   ← Public API barrel (curated exports only)
```

## How to Add a New Schema Field

1. **Write tests first** in `tests/unit/schema/task.test.ts` (or the relevant schema test file):
   - Test that valid values are accepted
   - Test that invalid values are rejected with correct error paths
   - Test that the field is optional (if it should be)

2. **Add the field** to the Zod schema in `src/schema/task.ts` (or relevant file)

3. **Update the TypeScript type** — it's auto-inferred from Zod, so just adding to the schema is enough

4. **Update `createTask()`** in `src/validation/factory.ts` if the field should have a default or be passthrough

5. **Update `CreateTaskInput`** type in `src/validation/factory.ts` to include the new field

6. **Regenerate JSON Schema**: `pnpm --filter @hippotask/core schema:generate`

7. **Update this AGENTS.md** if the field introduces a new pattern or gotcha

## Gotchas & Quirks

### `.refine()` for business rules
The HippoTask schema uses Zod `.refine()` for cross-field validation (e.g., `start_date <= due_date`). These run AFTER individual field validation. If you add a new cross-field rule, add it as another `.refine()` call — they chain.

### Required fields are minimal
Only 6 fields are required: `id`, `title`, `status`, `created_at`, `updated_at`, `schema_version`. Everything else is optional. This is intentional — keep the bar low for AI agents creating quick tasks.

### `priority_raw` is `string | number`
Because platforms use different types (Linear uses numbers 0-4, Jira uses strings like "High"). The Zod schema uses `z.union([z.string(), z.number()])`.

### `external_ids` values must be non-empty
The schema enforces `z.string().min(1)` on values in the `external_ids` record. Empty strings are rejected.

### JSON Schema is generated, not handwritten
`packages/core/schema/*.json` files are auto-generated from Zod via `zod-to-json-schema`. Never edit them directly — they'll be overwritten. Modify the Zod schema instead.

### `import type` for types, `import` for values
Use `import type { HippoTask }` when you only need the type. Use `import { HippoTaskSchema }` when you need the runtime Zod object. This is enforced by `verbatimModuleSyntax: true`.

### `.js` extension in imports
All relative imports must use `.js` extension: `import { foo } from "./bar.js"`. This is required by Node16 module resolution. TypeScript resolves `.js` to `.ts` at compile time.

## Testing Commands

```bash
pnpm --filter @hippotask/core test           # Run all core tests
pnpm --filter @hippotask/core test:watch     # Watch mode
pnpm --filter @hippotask/core build          # Build
pnpm --filter @hippotask/core schema:generate # Regenerate JSON Schema
```

## Public API Surface

Everything exported from `src/index.ts`. If it's not in `index.ts`, it's internal. Check `index.ts` before adding new exports — keep the surface area small.
