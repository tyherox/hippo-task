# ADR-009 — An opt-in UI for reviewing tasks before export

**Status:** Proposed — planning only · **Date:** 2026-10-06

## Context

The user wants a rudimentary, opt-in HippoTask UI with switchable Kanban and
list views, for quick edits before exporting tasks to CSV or a destination
such as Notion. The request admits planning this UI despite the earlier GUI
deferral in AGENTS.md and the product capsule. It does not authorize building
a hosted service or a general project-management application.

Today, `make ui` launches `playground/serve.py` and `playground/ui.html`.
That surface is a development playground: it builds the CLI, targets a scratch
store, switches between live and simulated data, exposes arbitrary CLI
arguments, and offers reset/seed controls. Its real-CLI approach is useful
precedent, but it is not the proposed real-project product surface.

The library already supplies task operations, field validation, claims,
history, description conflict handling, and a Notion CSV exporter. ADR-007
defines export bookkeeping; ADR-008 describes markdown and local media.
The current exporter supports field/label filters, but not an explicit set
of selected task IDs. Metadata updates do not yet have the description
editor's protection against stale edits.

## Working assumptions to confirm

- Saving an edit updates HippoTask; exporting is a separate, explicit action.
  An export-only copy of a task is not part of this proposal.
- The first handoff is a Notion-ready CSV imported manually. Direct Notion
  publishing is a later feature with its own decision record.
- Opt-in means the user explicitly runs `hippo-task ui`. Nothing starts in
  the background during normal CLI use. A separate installation or optional
  compile feature is not required for the first proposal.

## Feature admission

**Verdict: admitted for design within the scope below; implementation pending.**
The user and pain come from the current request. The historical capsule is
context, not a restriction overriding that request.

- [x] **User:** the person reviewing agent-produced tasks or preparing a
  backlog for handoff. This extends the human-plus-agent audience in the
  [product brief](../../.agents/product/product-brief.md).
- [x] **Pain:** short title, priority, field, and description edits are tedious
  through repeated commands or after importing into another tool. This is
  user-stated intent; the time saved still needs a dogfood trial.
- [x] **Fit:** a human interface over existing Task, Store, and Activity
  primitives, with the local store remaining authoritative. See the
  [domain map](../../.agents/product/domain-map.md) and its canonical-model
  correction pointing to AGENTS.md.
- [x] **Scope:** one store, board/list toggle, quick edits, selection, and
  reviewed CSV export. The
  [non-goals](../../.agents/product/non-goals.md) still inform the boundary
  around dashboards, permissions, and multi-provider sync.
- [x] **Reversibility:** unsaved edits can be discarded. Saved edits append
  ordinary events; correcting them appends more events and cannot erase
  history. Export creates a new file and records existing export events.
- [x] **Failure:** design stale edits, held tasks, validation errors, missing
  stores/media, unavailable server, failed writes, and changed export previews
  before implementation. Draft text stays visible after a failed save.
- [x] **Measurement:** dogfood a batch of roughly 20 tasks: switch views,
  edit five tasks, select a subset, and export exactly those tasks without
  hand-editing the CSV. Repeat while an agent edits one task; neither person's
  work may disappear silently. Compare the effort with the current CLI flow.
- [x] **Cost:** a browser surface and local HTTP boundary add packaging,
  accessibility, concurrency, and end-to-end testing work. Reuse library
  operations; keep one view model and one editor; defer integrations and
  independent draft storage.

## Proposed experience

1. Run `hippo-task ui` inside a configured project. It resolves the existing
   store, prints a local URL, and opens the browser. Provide `--no-open` for
   manual opening. The process runs until stopped. A missing store explains
   how to initialize one; opening the UI never creates it implicitly.
2. Show the project/store identity, search, filters, a **Board / List** toggle,
   **New task**, and **Export selected**. Both views retain the same filters,
   selection, and unsaved edits when switching.
3. Review and edit tasks. Clearly distinguish saved values from unsaved
   changes; saving writes to HippoTask. Export requires saved changes or an
   explicit discard, so an unsaved title cannot silently disappear from a CSV.
4. Select tasks, inspect the actual export rows, then save the export. The
   result identifies the CSV path, included count, skipped/changed tasks,
   and any accompanying media files.

| Surface | First-version behavior |
| --- | --- |
| Kanban | Todo, Doing, and Done columns; Cancelled available through an explicit toggle. Cards show number, title, priority, key fields, and blocked/holder indicators. |
| Moving a card | Drag between states, with a keyboard-accessible state menu. Commit on the explicit move action and report a refused move without losing the card. No within-column manual ordering. |
| List | Selectable rows; title, status, priority, assignee, labels, declared fields, and export status. Edit a row inline with Save/Cancel; Enter saves a single-line edit and Escape cancels it. |
| Shared task editor | A side panel for title, markdown description, priority, assignee, labels, and declared fields; existing blockers and history are visible. Save/Cancel apply to this task only. |
| Filters | Search plus status, priority, label, declared field, and export status. Show how many selected tasks are hidden by the current filter. |
| Export review | Show exact selected rows and counts: new, previously exported, and changed since export. Previously exported rows require an explicit include-again choice. |

The normal board groups by lifecycle state. **Blocked remains a badge/filter**
derived from dependencies, not a new state or a separate editable column.
Moving a card to Doing changes its state; it does not claim the task for the
human. Tasks held by another worker keep the existing state restrictions.
Metadata stays editable under the current collaborative rules. No automatic
force or reclaim is hidden in a drag action.

Save is per task, not a global transaction across the board. New-task creation
is a small explicit form. Bulk editing, relation editing, custom columns,
rich-text editing, file uploads, and persistent draft recovery can follow
only if the first workflow needs them. Existing media links are preserved.

## Export semantics

Reuse the existing Notion CSV profile first. A general spreadsheet CSV profile
is a separate small follow-up: its column/value choices and handling of cells
that spreadsheet software may interpret as formulas need an explicit contract.
Do not label the Notion profile as a universal spreadsheet export.

Preview is read-only. Only successfully saving the export file records the
existing `export` events. Keep the existing no-overwrite and failure-cleanup
behavior. Prefer saving to a known local path and showing that path; do not
treat starting a browser download as proof that a durable file was saved.
Media remains in a sibling `media/` directory according to the existing export
contract, with its current manual-attachment limitation surfaced in the result.

An export is not proof of import into Notion. Use **Exported** and **Changed
since export**, never **Synced**. Notion's current
[CSV documentation](https://www.notion.com/help/import-data-into-notion)
says CSV imports and merges add rows rather than updating existing rows.
The include-again action should explain that consequence.

The committed export must match what the person reviewed. Revalidate selected
IDs, rendered rows, relevant configuration, and export eligibility under the
store lock before writing. If they changed after preview, refresh the preview
and require the export action again. Checking only a task's content sequence
is insufficient: a blocker or export bookkeeping can change the output or
eligibility without changing that sequence.

## Proposed architecture

- Add a `hippo-task ui` command serving embedded static HTML/CSS/JavaScript
  through a small Rust HTTP adapter. Prefer plain browser code for this scope;
  users should need neither Python, Node, nor a source checkout.
- The adapter calls the Rust library operations; it does not write ledger
  events itself or implement a second task fold in JavaScript. Keep the demo
  playground separate.
- The browser uses one task collection and one shared editor for both views.
  Refresh periodically and on focus; retain unsaved values when new data
  arrives. WebSockets are unnecessary for the first version.
- Scope the server to loopback and the one resolved store. Use a per-launch
  session token and validate origins/hosts for mutations. Expose specific task
  and export operations, not the playground's arbitrary-command or reset API.
- Render task text safely. A markdown preview must not execute raw HTML;
  local media access is restricted to the store's media directory, and remote
  images are not fetched automatically. Keep credentials, analytics, and
  external assets out of the launch path.
- Use an explicit human UI context, with a non-personal default identity and
  per-session node; do not accidentally inherit an agent's claim identity.
  Choosing a personal actor name remains an explicit user decision.

Two additions to the library boundary are needed before editing/export UI:

1. **Conditional edits:** read a base task version and check it under the same
   lock used to update. Submit only changed fields. Initially, a stale metadata
   save may be refused with the draft preserved for comparison and retry.
   Preserve the existing paragraph-merge behavior for description editing.
   If metadata and body are saved together, validate both before appending any
   events; separate CLI calls must not masquerade as one atomic save.
2. **Exact export selection and review:** accept selected stable task IDs,
   return a structured read-only preview, and revalidate that preview when
   committing the export. Selection and destination rendering remain library
   responsibilities so CLI and UI can share them.

Choose HTTP and browser-opening dependencies during implementation against the
supported Rust version and packaging targets; do not build a custom HTTP parser.

## Conceptual integrity

**Mode: preserves.** The UI presents existing tasks and operations; it adds no
task state, claim type, event kind, export-only task copy, or alternate ledger.
A transient local HTTP process is an optional interface: CLI/library use stays
independent of it. The older capsule's broad GUI deferral is narrowed by the
user's explicit request, not by silently changing the task model.

**Risk and mitigation:** users could confuse a view switch with a data copy,
Doing with a claim, or exporting with synchronization. Shared task data,
distinct holder indicators, explicit Save/Export actions, and accurate export
labels preserve those distinctions. If later work adds persistent drafts or
direct publishing, revisit the model and this decision first.

## Implementation slices and acceptance

1. **Read-only UI:** command, store discovery, packaged assets, board/list,
   shared search/filter/selection, and task detail. Verify opening and viewing
   append no ledger events and missing stores are not initialized.
2. **Quick edits:** conditional update tests first, then inline/side-panel
   edits, creation, and state moves. Verify held-task rules, concurrent edits,
   invalid field values, lost connection, and retained drafts on failure.
3. **Reviewed export:** selected-ID and preview tests first, then the review
   flow and file save. Verify preview records nothing, export rows match the
   reviewed set, duplicate behavior is explicit, changed previews stop the
   export, and failed saves preserve existing rollback guarantees.
4. **Ship checks:** test the actual browser flow and packaged binary on the
   supported platforms; keyboard use and view switching must work. Update
   README, command/JSON documentation where applicable, and CHANGELOG. Run
   `make doctor` before implementation and `make verify` before completion.

This ADR records the proposed plan only. No UI, API, model, or export behavior
has been implemented by writing it.
