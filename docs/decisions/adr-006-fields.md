# ADR-006 — Fields: declared attributes, one value each

**Status:** Accepted · **Date:** 2026-10-03

## Context
The first team use is a product manager who breaks projects into tasks, sorts them by project and team, then uploads them to a permanent tracker (Notion first; ADR-007). Project and team aren't tags. Each task has *one* of each, the allowed values are a known list, and every tracker models them as a property or a container — Notion select properties, custom fields or lists in ClickUp and Asana.

Labels are the wrong carrier. They're a set by design, so "one project per task" would be a CLI rule layered on union semantics (merging two copies could leave a task in two projects), agents would parse `project:dashboard` out of strings, and validated categories would share a list with free tags. Tried on 0.4.1: a typo (`project:dashbaord`) and a case variant (`Team:Design`) went straight in, and one task ended up in two projects.

## Decision
1. **A task has fields**: named attributes with one text value each — `"fields": {"project": "dashboard", "team": "design"}` in every task object.
2. **Fields are declared** in the store's `config.toml`, next to `ledger.jsonl` — one table per field. `values` limits a field to a list; without it, any text goes. `display_name` names the field for people and exports (default: the name, capitalized):
   ```toml
   [fields.project]
   values = ["dashboard", "billing"]

   [fields.customer]        # no list: any text
   display_name = "Client"
   ```
   Undeclared fields are refused. Labels stay free-form and need no declaration.
3. **One new event kind, `set-field`**: `{"field": "project", "value": "dashboard"}`; `"value": null` clears it. The fold applies it like the title — the last write wins, per field — so two copies of a ledger merge to the same value in any order. Values are text: types such as a date or a number can be declared in config later without changing the event.
4. **Commands**: `add --field project=dashboard` (repeatable), `update --field project=billing`, `update --clear-field team`, `list --field project=dashboard` (repeatable; every one must match), and `hippo-task fields` — the declared fields, their allowed values, how many tasks use each, and *strays*: values tasks still carry that the config no longer allows. Writes refuse an undeclared field or a value off the list, and suggest the closest match. Clearing always works, so strays can be cleaned up; a filter on a value off the list warns but still runs, so they can be found.
5. **The rules are etiquette** (`ops.rs`). The fold accepts any `set-field` — a merge never refuses a fact — and only the CLI reads the config.
6. **Labels get a filter too**: `list --label bug` (repeatable). They stay free tags; now they're findable.

## Alternatives
- **`key:value` labels plus a vocabulary** (like GitLab's scoped labels or Linear's label groups). Rejected for the reasons above: one-value would be etiquette over a set, merges could keep two values, and agents would parse strings.
- **First-class `project` and `team` attributes.** Rejected: teams slice work differently (customer, area, quarter). A declared map handles any dimension without a schema change for each one.
- **The vocabulary as events in the ledger.** Deferred: a file is easier for a person or an agent to write in bulk and to review. The values on tasks are events either way.
- **Multi-valued fields** (Asana lets a task sit in several projects). Deferred: labels cover "many". If a declared field ever needs several values, it gets its own event kinds, and `set-field` keeps its shape.

## Consequences
- 0.4.x can't read `set-field`. It skips those lines with a warning — which now says the ledger was written by a newer hippo-task — so this ships as **0.5.0**, and everyone sharing a store upgrades together.
- The config is per store, and the store's own `.gitignore` keeps it out of git along with the tasks. Sharing it through git is the same choice as sharing the tasks.
- Changing the config never changes tasks: they keep their values, `fields` lists the strays, and `update --field` or `--clear-field` fixes each one.
- Syncing with trackers, later, maps each field to a provider property or container in config. Provider ids and sync state get a place of their own, never fields.

## Amendment — 2026-10-05, before release
1. **Settings this version doesn't know are warned about, not refused** — at the top of `config.toml` only. Later versions will add sections (sync, export profiles), and a store's config is shared by everyone using it, so an older binary must keep working: it names the setting, suggests the closest one it knows (`feilds` → `fields`), and ignores it. Inside a field's table an unknown setting is still an error — that's where typos like `vaules` happen, and no later version is expected to add settings there.
2. **A filter names a field once.** `list --field project=a --field project=b` could never match — a field holds one value — so it's refused, like setting a field twice.
