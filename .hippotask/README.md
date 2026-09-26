# `.hippotask/` — this repo's own task ledger

`hippo-task` keeps a project's tasks in `.hippotask/ledger.jsonl` inside that project: one JSON event per line, append-only (format: `docs/schema-design.md` §11). This folder is where the ledger for *this* repo lives if you run `hippo-task` on itself; in git it holds only this README.

Everything except this README is gitignored, so agent task state stays local by default.

**Sharing tasks through git** is possible because the fold doesn't depend on line order: remove the two `.hippotask` lines from `.gitignore`, and add `.hippotask/ledger.jsonl merge=union` to `.gitattributes` so concurrent appends merge as a union instead of conflicting. Multi-machine sync is not a supported feature yet, though: timestamps are only guaranteed to increase within one machine.
