# ADR-004 — Duplicates: find before filing, mark when found

**Status:** Accepted · **Date:** 2026-09-26

## Context
A claim stops two workers from doing the same *task*. It can't stop them doing the same *work* when that work was filed twice. In real use, one agent filed 36 fix tickets from five reviewers' findings in a single second, and 7 of them (19%) were the same defect filed again. The fixing agent only found out after claiming each one — "Fixed with ticket 54 (same defect)" — then closed it as `done`. So the duplicates cost a claim each, counted as finished work, and recorded their link to the original only in note text. With several fixers working in parallel, two of them would have fixed one defect in the same file at the same time.

Every duplicate pair shared a distinctive term in its title — a function name, a file, a metric. And the agent recognised each duplicate at once when it read the ticket. What was missing was a way to *find* candidates before filing, and a way to *record* the verdict.

**Automatic detection was tested and rejected.** Warning on `add` when an open task's title shares words with the new one, replayed over 160 real tasks: catching all 7 duplicates meant warning on 41–48% of all `add`s, even with words weighted by rarity; cutting the noise to about 15% missed 3 to 5 of the 7. In a real backlog, related tasks share vocabulary, so word overlap measures *topic*, not *sameness* — and a warning that fires on half of all adds teaches agents to ignore it.

## Decision
1. **Find before filing: `hippo-task list --search <text>`.** Tasks whose title or description contains the text, ignoring case; repeat `--search` to require several terms. Any state, since the original may already be done. A plain substring, not a pattern, so what an agent types is what it finds.
2. **Mark when found: `hippo-task update 80 --duplicate-of 55`.** Records a new `duplicate-of` relation and cancels the task, in one step — so a duplicate never counts as done work, and `show` links it to the original. Closing etiquette applies as usual: while someone else holds the task, only they can mark it (or a person with `--force`).
3. **AGENTS.md makes it part of the loop:** before `add`, search for the most distinctive term in what you're about to file — a function, a file, an error message. If a task already covers it, add a note there instead. If you find you're working on a duplicate, mark it.

## Alternatives
- **Warn on `add` about similar titles.** Rejected on the numbers above; revisit only with a signal better than word overlap.
- **Semantic (embedding) search.** Deferred: it needs a model and an index, and plain search plus the agent's own judgment covers every duplicate seen so far.
- **Mark duplicates with the existing `relates-to` plus a note.** Rejected: "duplicate" would live only in free text again, and it's a standard relation across trackers (Jira, GitHub, Linear).
- **Search notes too.** Deferred: notes live in the history, not the task, and titles plus descriptions held every signal so far.

## Consequences
- The model gains the relation `duplicate-of`; the `relate` event keeps its frozen shape. 0.2.x can't read the new relation (it skips such lines with a warning), so this ships as **0.3.0** and everything sharing a ledger upgrades together.
- `--duplicate-of` refuses the task itself as its own original. To undo a wrong mark, reopen the task (`update 80 --state todo`); removing the link is deferred until someone needs it.
- Search is a read, so it isn't recorded; the ledger shows only what agents did after searching.
- Plain `list` still shows closed tasks, duplicates included. Hiding them by default is a separate candidate.
