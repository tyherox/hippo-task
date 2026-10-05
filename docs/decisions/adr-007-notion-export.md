# ADR-007 — Upload to Notion: a CSV export that remembers

**Status:** Accepted · **Date:** 2026-10-03

## Context
The first team use is a product manager who drafts and organizes tasks locally (ADR-006), then uploads them to Notion as their permanent home. Two-way sync is later work: it needs per-field change tracking, id mapping and API rate limits. Notion imports a CSV as a new database, or merges one into an existing database when the headers match its property names exactly. Imports only add rows — Notion's help says "Imports add rows. They don't update existing rows, so watch for duplicates" — so uploading a task twice duplicates it.

## Decision
1. **`hippo-task export notion`** writes CSV: UTF-8, a header row, one row per task, quoted where needed (RFC 4180). Columns: `Name` (the title — first, so a fresh import makes it the page title), `Status`, `Priority`, one column per declared field (its `display_name`, in field-name order), `Tags` (labels, comma-separated, the way Notion reads a multi-select), `Assignee`, `Description`, `Blocked by`, `hippo-task ID`.
2. **Values in Notion's terms.** Status `todo` → `Not started`, `doing` → `In progress`, `done` → `Done` (Notion's default status options), `cancelled` → `Cancelled`. Priority `urgent` / `high` / `med` / `low` → `Urgent` / `High` / `Medium` / `Low`; `none` → blank. `Blocked by` lists each blocker as `#3 Title`, separated by `; `. `hippo-task ID` is the ULID, so a row can always be traced back — and matched by a future sync.
3. **Which tasks**: open tasks not yet exported to Notion. `--field` and `--label` narrow it (repeatable); `--all` adds closed tasks; `--again` adds tasks already exported.
4. **It remembers — but only when it writes a file.** `--out tasks.csv` writes the file and records an `export` event on each task (`{"to": "notion"}`); the task's `exported` object then says when (`{"notion": <unix ms>}`). Without `--out`, the CSV goes to stdout as a preview and nothing is recorded, so a preview can never hide a task from the next real upload.
5. **Edited after upload?** The report lists exported tasks that changed since — to fix in Notion by hand, or to re-add as new rows with `--again`.
6. **`export --json` needs `--out`**: the CSV goes to the file, and stdout gets `{"file", "exported", "changed"}` — the file (`null` when nothing was new), the exported task objects, and the ones left out because they changed since.
7. **`--out` never overwrites a file.** If an earlier export hasn't been imported yet, overwriting it would lose tasks already marked as exported.

## Alternatives
- **The Notion API.** Deferred to sync: it needs an integration token, page-by-page writes (Notion allows about three requests a second), and the change tracking sync needs anyway. A CSV needs no key, no paid plan, and uses Notion's own importer.
- **Always record, with a preview flag.** Rejected: forgetting the flag would mark tasks uploaded that never were — they'd silently never reach Notion. Recording only with `--out` errs toward a visible duplicate instead.
- **Mark exported tasks with a label.** Rejected: labels are the team's words. Bookkeeping gets its own event and attribute.
- **A generic CSV.** Deferred: column names and status values are destination-specific. `notion` is the first profile; others can follow the same shape.

## Consequences
- The model gains the event kind `export` and the task attribute `exported`. As with ADR-006, 0.4.x skips the new lines with a warning, and everyone sharing a store upgrades together.
- An export is bookkeeping, not a change: `seq` and `updated_ms` stay as they were.
- Assignees export as recorded (`human:ana`). Matching people to Notion accounts waits for sync — that mapping is personal data, so it will live in local config, never in the ledger.

## Amendment — 2026-10-05, before release
Review of the first build found three gaps; the fixes change no event shape.
1. **"Changed" means the row changed.** Comparing `updated_ms` with the export flagged a task after a mere note (not in the CSV) and missed a blocker closing (the `Blocked by` cell changes, the task's `updated_ms` doesn't). Instead the export folds the ledger as it stood at each task's last export, renders that row, and compares it with the row now — derived, like `blocked`, so nothing new is stored. One fold per export batch: an export changes no content, so the tasks of one batch share one fold.
2. **Closed tasks are checked too.** A task done after its upload is the commonest change, and `--all` used to be the only way to see it. Every exported task that matches `--field`/`--label` is checked; `--all` still decides only whether closed tasks are *exported*.
3. **Column headings are unique.** A field whose `display_name` repeats a built-in column (`Status`, `Tags`, … — compared ignoring case) or another field's makes `config.toml` invalid: duplicate headers break Import and Merge with CSV. Set a `display_name` to resolve it.
4. **A failed record takes the file with it.** If recording the export fails after the file was written, the file is deleted, so no file exists for an export the ledger doesn't know about.

Considered and left out: prefixing cells that start with `=`, `+`, `-` or `@` with `'` to stop spreadsheet formulas. Notion doesn't evaluate them, and the prefix would show up in Notion titles — the file's only purpose. Revisit if a spreadsheet profile is added.
