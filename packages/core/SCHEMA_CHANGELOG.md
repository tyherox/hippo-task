# HippoTask Schema Changelog

Every HippoTask object carries a `schema_version` field. This file documents every version the registry can parse, what changed between versions, and how payloads migrate forward. The registry itself lives at [`src/versioning/registry.ts`](src/versioning/registry.ts).

## Wire-format version vs. package version

The `schema_version` field identifies the **wire format** — the shape of a serialized HippoTask — not the npm package release. These are decoupled:

- `@hippotask/core@1.0.3` (package version) may still emit `schema_version: "1.0.0"` because nothing about the wire format changed.
- The registry only lists entries for distinct wire formats. A patch release of the package doesn't add a registry entry.
- Adapters, MCP clients, and round-trip tooling all match on `schema_version`, so keeping it stable across no-op package patches is the right behavior.

## When to bump `schema_version` (add a registry entry)

Versioning follows [Semver](https://semver.org):

- **Major** (`2.0.0`) — breaking: required fields removed/renamed, enum values removed, field types narrowed. Requires a migrator.
- **Minor** (`1.1.0`) — additive: new optional fields, new enum values. Requires a migrator only if new fields are required (with a default).
- **Patch** (`1.0.1`) — no wire-format change. Docstring edits, error message tweaks, stricter runtime refinements. **No registry entry**, no `schema_version` bump — these go out as a package patch release and old payloads continue to validate.

## Versions

### 1.0.0 — current

Initial schema. Defines `HippoTask` (id, title, status, created_at, updated_at, schema_version required; ~20 optional fields), `HippoProject` (id, name, timestamps, schema_version required), `HippoPerson`, `HippoCustomField`, and the closed-set enums for status, priority, description format, estimate unit, project status, and custom field type.

No migrator — this is the chain's origin.

---

## How to add a new version

1. **Decide if you need a new version at all.** Patch-level changes (docstrings, refinements that don't reject previously-valid payloads) don't. Only bump the registry when the wire format changes.

2. **Pick the version number** per the semver rules above.

3. **Write the migrator.** It is a pure function `(previous: unknown) => unknown` that upgrades a document from the immediately prior version's shape to the new shape. The input is untrusted — treat it as `unknown` and narrow safely. The output does not need to be validated inside the migrator; `migrateTask` / `migrateProject` run Zod validation after the full chain completes, so a migrator bug surfaces as a validation error with a clear path.

4. **Append to the registry.** Add the new entry as the last element of `REGISTRY` in [`src/versioning/registry.ts`](src/versioning/registry.ts). Order is load-bearing — oldest → newest.

    ```ts
    export const REGISTRY = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => {
          const doc = prev as Record<string, unknown>;
          return { ...doc, owner: "unknown" };
        },
      },
    ] as const satisfies readonly VersionRegistryEntry[];
    ```

5. **Update the Zod schemas** (`src/schema/task.ts`, `src/schema/project.ts`) to match the new shape. `CURRENT_SCHEMA_VERSION` and `SUPPORTED_VERSIONS` update automatically from the registry — don't edit them by hand.

6. **Write tests:**
   - A happy-path migration: a fixture at the old version upgrades cleanly.
   - Idempotence: upgrading a current-version payload is a no-op.
   - Breakage: a malformed input raises `UnsupportedSchemaVersionError` or `ZodError` with a readable message.

7. **Regenerate JSON Schema:** `pnpm --filter @hippotask/core schema:generate`. The `$id` and `title` of the output files pick up the new version.

8. **Document the change here.** Append a new section above (newest version on top after 1.0.0). Include: what changed, rationale, migration notes, any breaking consumer-visible behavior.

9. **Update adapter capabilities** (once adapters exist): adapters declare which schema versions they can produce and consume. Bumping the schema may require adapter work — tag `@hippotask/adapter-*` packages for review.

## What migrations must guarantee

- **Lossless when possible.** Preserve original data via `metadata` or `*_raw` fields when the new shape can't hold it directly (see [`docs/SCHEMA.md`](../../docs/SCHEMA.md)).
- **Deterministic.** Same input → same output. No `Date.now()`, no randomness, no network calls.
- **Total.** Every payload that was valid at the prior version must be migratable. If a migration genuinely can't handle some prior-valid payload, the migrator should throw a clear error, not produce a malformed result.
- **Pure.** No mutation of the input. No external side effects.
