# ADR-010 — Move through tasks without leaving the reader

**Status:** Accepted for implementation · **Date:** 2026-10-06

The user reviewing a real, screenshot-rich backlog asks for easier movement
between open tasks and an interface that works for nontechnical reviewers.

Keep ADR-009's local, explicit-save model. Add a compact task list alongside a
wider editor, Previous / Next, a position indicator and task chooser, and
Save & next. Follow the current filtered list (column order in Board view).
At the ends, disable unavailable actions. If the current task is filtered out,
say so and let the chooser select a matching task. Normal navigation retains
each task's unsaved draft in this tab. Save & next moves only after a confirmed
save; errors and conflicts keep the reviewer on their draft. Capture the next
task before saving, since saving can change filter membership.

Open existing descriptions in their formatted view. Use everyday labels and
put organization fields and technical workspace details behind disclosure.
Keep the exact Markdown source and existing safe renderer; no new document
model or rich-text dependency. Keep keyboard focus predictable and provide
Alt+Left/Right navigation outside text controls, plus Escape to close.

Make existing inline images discoverable in a screenshot strip with an
enlarged, keyboard-accessible viewer. Rename Add media to Add screenshots / files
and accept an explicitly pasted clipboard image while the editor is open.
Use the same authenticated upload/preview routes, size checks, generic captions,
and explicit Save. Text pastes remain text. No clipboard polling or automatic
capture; no remote image loading, filename collection, or file deletion.

Failures must preserve drafts, cancel stale preview responses, keep uploads
bound to their original task, and explain unavailable screenshots. Read-only
navigation and image viewing write no events. Existing source screenshots are
reused; this change does not invent or overwrite task requirements.

Verify navigation boundaries/filter changes, unsaved return, successful and
failed Save & next, navigation during requests, and clipboard ownership with
behavior tests. Test the shipped UI against the real sample backlog on desktop
and a narrow viewport, including image viewing and keyboard access. Run
`make verify`; a small Node built-in test suite supplements Rust HTTP tests
without adding a browser or package dependency to the shipped application.
