# ADR-011: Compact workspace and explicit bulk editing

Status: accepted, 2026-10-06

## Problem

The review UI is cumbersome for a real backlog: large cards and forms hide the
work, opening a task replaces the list, and selection only supports exporting.
The user requested an overhaul inspired by Linear, including bulk status edits.

## Decision

- Keep the existing vanilla browser UI and authenticated local API. Use a compact
  workspace shell, list-first rows, optional board, search, sort and tucked-away
  filters. A right-hand detail panel preserves the list and selection.
- Checkboxes, select-all-visible and Shift-click ranges share one selection in
  both views. Explicitly count selected tasks hidden by filters.
- A contextual toolbar changes status, priority, owner, tags or a declared field.
  Choosing a value does not write; **Apply to N tasks** starts the operation.
  Removing a value (**Unassign owner**, **Clear** *field*) is its own choice in
  the property menu: a blank or untouched value control never clears anything.
  The toolbar takes the place of the selection counts in their row, so
  selecting never moves the list. Rows show who holds a task, since held tasks
  keep their status in a bulk change.
- Capture IDs, base sequences and desired patches before starting. Use the
  existing conditional task PATCH endpoint, sequentially. Never force a held
  task, rebase an outdated task silently, or overwrite an unsaved draft.
- Report updated, unchanged and failed tasks separately. Preserve selection so
  people can make another edit. Failures remain individually reviewable. Stop
  after an uncertain connection failure and mark remaining tasks unattempted;
  do not retry automatically or claim atomicity across tasks.
- Allow undo of the most recent successful bulk changes, using the returned
  sequence for each task. A subsequent edit must make undo stale, not overwrite
  newer work. A batch that changes nothing keeps the previous undo; running undo
  uses it up. Do not add new event types or modify the ledger directly.
- Preserve Markdown preview, screenshot upload/paste/viewer, draft navigation,
  conflict handling and reviewed CSV exports. Put the task description ahead of
  optional properties; keep actions and navigation reachable without page scroll.

## Verification

Write behavioral tests before implementation: filtered/range selection, sorted
navigation, conditional multi-task changes, held/dirty/stale tasks, partial
failure, uncertain connection results, and conditional undo. Exercise actual
bulk writes in a disposable copy of the sample workspace, then open the real
workspace without changing its tasks. Inspect desktop and mobile layouts and
run `make doctor` and `make verify`.
