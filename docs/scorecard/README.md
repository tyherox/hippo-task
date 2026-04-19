# Scorecard — contributor guide

This directory is the **source of truth** for the HippoTask provider scorecard. Each provider has one YAML file in [`providers/`](providers), validated against the [rubric](../SCORECARD_RUBRIC.md) by a deterministic generator.

- **Rubric** (the rules): [`../SCORECARD_RUBRIC.md`](../SCORECARD_RUBRIC.md)
- **Source data** (one per provider): [`providers/*.yaml`](providers)
- **Generated outputs** (do not edit by hand):
  - [`../PROVIDER_SCORECARD.md`](../PROVIDER_SCORECARD.md) — autogen sections bounded by `<!-- BEGIN:... -->` / `<!-- END:... -->`
  - [`../scorecard.json`](../scorecard.json) — machine-readable copy
- **Audit log**: [`../scorecard-history.json`](../scorecard-history.json)

## Commands

| Command | What it does |
|---|---|
| `pnpm scorecard:generate` | Rebuild `scorecard.json` + splice autogen sections into `PROVIDER_SCORECARD.md`. Run after any YAML change. |
| `pnpm scorecard:validate` | Zod-validate every YAML, re-run score math, and check that the generated artifacts on disk match what would be regenerated (CI gate). Exits non-zero on any drift. |

The generator is deterministic — `pnpm scorecard:generate` followed by `pnpm scorecard:validate` should always succeed on a clean checkout.

## Adding a new provider

1. **Copy** an existing YAML as a template — `providers/linear.yaml` is a good reference (all sub-criteria met, clear prose).
2. **Rename** to `providers/<slug>.yaml`. The slug must be lowercase kebab-case and match the `slug:` field inside.
3. **Audit each of the 8 dimensions** against the [rubric](../SCORECARD_RUBRIC.md) §3:
   - **Sum dimensions** (D1 / D2 / D6 / D8): list every sub-criterion in the exact order defined by the rubric (`task_crud`, `container_crud`, … — the schema rejects wrong order). Mark `met: true` only if the vendor's own docs support it.
   - **Band dimensions** (D3 / D7): pick the single highest band whose criterion the evidence satisfies. Write a `justification` sentence citing the vendor doc.
   - **D4 Rate Limits**: record `effective_ops_per_min` using the paradigm formula from rubric §3 D4. The `formula` field should show the math so another auditor can replay it. The band is derived automatically.
   - **D5 Real-Time**: record `event_count` (count of vendor-documented webhook event identifiers — see rubric §3 D5 counting rule). Pick a `band`. For band ≥ 3, the validator checks `band` against the event-count ceiling.
4. **Citations.** Every sub-criterion with `met: true` must have an `evidence:` URL pointing at a canonical vendor doc. The validator enforces this. Marketing pages don't count — cite the API reference, changelog entry, or rate-limit page that most directly supports the claim.
5. **Run** `pnpm scorecard:generate` — this updates the table, detailed ratings, and `scorecard.json`. Review the diff before committing.
6. **Run** `pnpm scorecard:validate` — fails on any drift or schema violation.
7. **Log** the audit: add a `provider_deltas` entry in [`../scorecard-history.json`](../scorecard-history.json) under the current audit date. If this is the first time scoring the provider, `from: null, to: <overall>`.

## Updating an existing provider's score

1. Edit the provider's YAML. Move only cells where the evidence has actually changed (new MCP server, new rate limit, deprecated endpoint, etc.).
2. Update `verified_at` to today's date if this is part of a wider re-audit (all providers must share the same `verified_at` — the generator rejects mixed dates).
3. Run `pnpm scorecard:generate` and review the diff.
4. Append an audit entry to `scorecard-history.json` with `provider_deltas` for every cell that moved.
5. Bump the scorecard version (patch for corrections, minor for new audit cadence, major for rubric-version-triggered re-audit).

## YAML schema reference

The Zod schema lives in [`../../packages/core/scripts/scorecard/schema.ts`](../../packages/core/scripts/scorecard/schema.ts). At a glance:

```yaml
name: string             # Display name (e.g., "Monday.com")
slug: string             # kebab-case, must match filename
verified_at: YYYY-MM-DD  # Must match every other provider's date
bottom_line: string      # One-paragraph editorial summary shown in §3
gotcha: string           # One-line biggest-gotcha for the §5 table

scores:
  api_completeness: { sub_criteria: [5 items in rubric order] }
  api_ergonomics:   { sub_criteria: [5 items in rubric order] }
  schema_flexibility: { band: 1-5, justification: string, evidence: url? }
  rate_limits: {
    effective_ops_per_min: number, tier_scored: string,
    formula: string, justification: string, evidence: url?
  }
  real_time: {
    band: 1-5, event_count: int,
    justification: string, evidence: url?
  }
  auth_simplicity: { sub_criteria: [5 items in rubric order] }
  data_portability: { band: 1-5, justification: string, evidence: url? }
  ai_agent_readiness: { sub_criteria: [4 items in rubric order] }
```

Each sub-criterion is `{ id: string, met: bool, evidence?: url, note: string }`. The `note` is the prose rendered in `PROVIDER_SCORECARD.md` §3 — use inline markdown freely (links, code, emphasis). The `id` must be the exact rubric identifier (e.g., `task_crud`, `vendor_sdk_fresh`). The validator rejects wrong order or unknown IDs.

## CI integration

Drop this into any CI pipeline:

```yaml
- run: pnpm install --frozen-lockfile
- run: pnpm scorecard:validate
```

It returns zero iff:
- every YAML passes the Zod schema,
- every `met: true` sub-criterion has an evidence URL,
- all providers share the same `verified_at`,
- the on-disk `scorecard.json` + `PROVIDER_SCORECARD.md` autogen sections match what the generator would produce from the current YAMLs.

If drift is detected, the validator points at which file or marker is stale — usually the fix is `pnpm scorecard:generate && git add -u`.
