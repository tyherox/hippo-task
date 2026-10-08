# ADR-013 — Similar tasks, ranked, on request

**Status:** Accepted · **Date:** 2026-10-08

## Context
ADR-004 asks agents to search before filing (`list --search <term>`) and rejected warning on every `add`. Replayed over the pilot, catching all 7 duplicates meant warning on 41–48% of adds. Search still depends on the agent picking the right term, and on it searching at all: the agent that filed those 7 filed every finding without checking.

The request this time was a check for *identical* tasks. Replayed over the pilot's 228 tasks, an identical-title check (ignoring case, punctuation and spacing) finds **0** pairs, so it would have caught none of the 7. Duplicates are restatements, not copies: the same defect, described in different words by different reviewers.

A *ranking* does catch them. For each task in filing order, rank every earlier task by the distinctive words they share. Each shared word is weighted by how rare it is in the store, ln((N+1)/(df+0.5)), and identifiers like `session_token` and `settings.yaml` stay whole:

| compared on | original ranked #1 | in the top 3 | in the top 5 |
|---|---|---|---|
| titles | 6 of 7 | 7 of 7 | 7 of 7 |
| titles and descriptions | 7 of 7 | 7 of 7 | 7 of 7 |

That's one project and seven pairs, too few to set a threshold for a warning. It is enough to put the right candidate in front of the agent, which then judges. The agent in the pilot recognised every duplicate at once when it read the ticket.

## Decision
1. **`hippo-task similar <text> [--limit N]`** (default 5): tasks ranked by the distinctive words they share with the text, best first. It compares titles, descriptions, and image captions (file paths are skipped, as `--search` skips them). It covers any state, since the original may be done. It skips tasks already marked `duplicate-of`, so their original is what surfaces. `--json` prints an array of task objects, the same shape as `list`; with nothing in common, `[]`. Scores aren't printed, so the ranking can improve without a contract change.
2. **Words**: lowercase runs of letters, digits, `_`, `.`, `/` and `-`, trimmed of edge punctuation, without one-letter words and a short stop list. Each shared word counts once, weighted as above over the store's tasks.
3. **The loop changes.** Before `add`, run `similar` with the title and description you're about to file and read what comes back. If one covers it, add a note there instead. `list --search` stays for exact lookups.
4. **`add` warns about an identical open title.** The comparison ignores case, punctuation, and spacing. It almost never fires wrongly, so it can run on every add. The task is still created, because recurring work ("Weekly release notes") legitimately repeats.

## Alternatives
- **Warn on `add` with the candidates.** Rejected again for ADR-004's reason: any threshold tuned on seven pairs either nags or misses.
- **Put the candidates in `add`'s output.** Rejected: by then the duplicate exists, and undoing it costs a second command.
- **BM25 with length normalization, or embeddings.** Deferred: the plain weighted sum already ranks every known duplicate first, and embeddings need a model and an index.
- **Refuse identical titles.** Rejected: recurring tasks would need an override flag, for a case the pilot never hit.

## Consequences
- Read-only: no events, no stored index. Each call reads every task, as `list` does. At about 1 ms per 1,000 events, that's well under a human's notice.
- `duplicate-of` relations (ADR-004) now record every marked duplicate, so the next replay can take its ground truth from the ledger instead of from notes. Re-measure when there are 20 or more pairs, before considering any warning.
- Filing many tasks at once (the pilot's 36 tickets in one second) is covered when each `add` is preceded by `similar`. A bulk add from a plan file, with candidates checked across the batch, is a separate decision.
