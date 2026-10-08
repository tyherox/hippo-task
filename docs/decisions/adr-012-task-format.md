# ADR-012 — Task format: a team's conventions, in its config

**Status:** Accepted · **Date:** 2026-10-08

## Context
Teams write tasks differently. One wants a reason and a "done when" checklist on every task, another wants repro steps on bugs; one requires a project on every task, another doesn't care. ADR-006 fields check a task's *values*, but nothing tells an agent or a PM what a good task looks like *here*: the guide is the same for every project, so each agent invents its own shape.

The pilot project's backlog shows the drift. All 228 tasks have a description, but none uses headings, and only 28 (12%) contain "done when", "acceptance" or "definition of done". The conventions existed only in the head of the person reviewing the backlog.

## Decision
1. **A store's `config.toml` may declare a format.** Every key is optional:
   ```toml
   [format]
   guide = """
   Titles start with a verb and name the thing: "Fix token refresh on 401".
   One outcome per task. If the title needs "and", split it.
   """
   template = """
   ## Why

   ## Done when
   - [ ]
   """
   required_fields = ["project"]
   required_sections = ["Done when"]
   ```
   `guide` is prose for whoever writes tasks. `template` is the description to start from. `required_fields` must name declared fields (ADR-006). `required_sections` are headings the description must have, with something under them. When a template is set, each one must be a heading in it.
2. **`hippo-task format` shows it** (`--json`: `{"config", "guide", "template", "required_fields", "required_sections"}`, with `null` or `[]` for what isn't set). The agent guide says to read it once per session before filing tasks.
3. **Writes warn, never refuse.** After `add`, `desc`, or an `update` that changes the description or a field, an open task that misses a requirement gets one `{"warning": …}` per gap, naming the fix (`hippo-task desc 3 --base …`, `--field project=…`). The write still happens. Other updates (priority, state, labels) don't check: a PM triaging the backlog isn't shaping the task. If the config can't be read after a write, that's a warning too, never a failed command.
4. **A section counts as filled** when its heading, at any level and ignoring case and a trailing colon, is followed by at least one line of text before the next heading of the same or a higher level. A bare `- [ ]` or an HTML comment doesn't count, so an untouched template still warns.
5. **The UI starts new tasks from the template** and marks missing required fields. Its state gains `format`.
6. **It's etiquette** (`ops.rs`), like fields. The fold never reads the config, the ledger never refuses an event, and task objects don't change.

## Alternatives
- **Refuse writes that miss the format (exit 2).** Deferred. An agent filing a finding in the middle of other work would lose it or retry blindly, while a warning names what's missing and the task can be fixed with `desc`. If a team wants enforcement, `strict = true` can come later without changing anything here.
- **`required = true` on a field's own table.** Rejected: since the ADR-006 amendment, an unknown key inside a field's table is an error, so everyone on an older binary would lose the whole store until they upgrade. A new top-level section is only warned about and ignored by older binaries.
- **Write the template into every task on `add`.** Rejected: empty headings in every task an agent files tell the next reader nothing. Whoever writes the task sees the template; the UI pre-fills it for people.
- **The template in its own markdown file.** Rejected for now: one file to share and review, and TOML's `"""` strings hold markdown as-is.
- **A template per kind of task** (bug, feature). Deferred until a team asks. It would be `[format.<kind>]`, chosen by a label or a field.

## Consequences
- No new event kinds and no change to the task object, so 0.7.x readers are unaffected. Older binaries warn that `format` is a setting they don't know, and ignore it.
- Inside `[format]`, unknown keys are warned about with the closest match (`requried_fields` → `required_fields`), not refused, because later versions may add keys there.
- The docs tests apply: `hippo-task format` appears in README.md, and its JSON keys in AGENTS.md.
- Listing existing tasks that miss the format (a backlog check) is a separate decision. The rule in 4 is what it would reuse.
