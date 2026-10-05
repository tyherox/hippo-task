# ADR-008 — Descriptions in markdown, with images and video

**Status:** Proposed · **Date:** 2026-10-06

## Context
A task's description is one optional string — `add --body`, `update --body`, `hippo-task desc` — printed back as plain text. Bug reports and design tasks need a screenshot, a GIF, or a short screen recording next to the words that explain it. Two things stand in the way. The ledger is text, one JSON event per line, read and folded by every command, so files can't live in it. And `desc` replaces the whole description: when two people edit it at once, one edit silently disappears — last writer wins, as with the title.

A first draft stored the description as blocks — paragraphs and files with ids, four new event kinds, ordered by "follows block X" links — so that two copies of a ledger could merge edits to different paragraphs. Review found it needed tombstones, rules for sibling order and for moves that form a cycle, a rule for matching edited paragraphs to block ids, and a forced upgrade. It paid all that for two things that don't exist yet: syncing copies of a ledger, and per-paragraph permissions. Today every edit goes through one ledger file under one lock, so a merge at write time covers it.

## Decision
1. **The description is markdown**, stored where it is today: the `body` of `create` and `set-body`. **No new event kind.** The CLI stores and prints the source; it never renders HTML.
2. **Files live beside the ledger, named by their content**: `<store>/media/<sha256>.<ext>`. The markdown links to them by that store-relative name — `![the 401 from the refresh call](media/9f2c….png)` — so the text is the same on every machine, and two links to the same bytes share one file. Nothing else about a file is recorded: not its original name, not its folder.
3. **Types are read from the leading bytes**, never from the file name: PNG, JPEG, GIF and WebP images; MP4, QuickTime (`.mov`, what macOS screen recording writes) and WebM video. MP4 and QuickTime share a container with HEIC and AVIF, so the `ftyp` brand decides; WebM shares one with Matroska, so the EBML doc type decides. Anything else is refused (exit 2), saying what the file is when that's known.
4. **Writing.** `desc 3 "<markdown>"`, `desc 3 --file note.md`, or `--file -` for stdin; `add --body` and `update --body` take the same markdown. In each image link, `![caption](target)`:
   - `media/<sha256>.<ext>`, or a path into this store's `media/` folder, is kept as that name — with a warning if the store doesn't have the file;
   - `http:`, `https:` and other URLs stay as text and are never fetched;
   - any other target is a local file, relative to the `--file`'s folder, otherwise to the current folder (`<…>` brackets a path with spaces). It is checked, copied in, and the link rewritten to its store name. A missing file is refused, by name.

   Image syntax inside code — a fenced or indented block, or a `code span` — is text. The command refuses an empty file, anything that isn't a regular file, and a file over the cap: 8 MiB for an image or GIF, 64 MiB for a video. `[media]` in `config.toml` raises the caps (`max_image_mib`, `max_video_mib`).

   Files are hashed and copied before the ledger is locked — to a temporary name in `media/`, flushed, renamed, and the folder flushed — so a large video never holds up other writers, and the event is never on disk before its file. A file already under its name is the same bytes and is left alone. If the command fails after copying, the file stays: an unreferenced file is harmless, and a command never deletes one, because another writer may be about to link the same bytes. Cleaning them up is later work.

   Text is normalized before it's stored: LF line endings, no blank lines at either end, one blank line between blocks outside fenced code.
5. **Merging concurrent edits: `desc --base <seq>`.** The caller passes the task's `seq` from its last read. If the description hasn't changed since, the new text is written. If it has, the command merges three versions by paragraph (blocks split at blank lines outside fenced code): the description at that `seq`, the submitted text, and the description now, under the ledger lock. A paragraph only one side changed — edited, added, or removed — takes that side's version; one both sides changed the same way is kept once. Paragraphs both sides changed differently are a conflict: nothing is appended, and the command exits **5 (`stale`)**, naming them, so the caller re-reads the task and redoes the edit. Without `--base`, the text replaces the description, as today. A `--base` above the task's `seq` is a usage error (exit 2).
6. **The description stays collaborative**, like the title: no holder check (`guard_holder` keeps state changes and closing for the holder). `--base` is what prevents a silent overwrite.
7. **Reading.** The task object gains `media`: one entry per link, in reading order, with `caption`, `sha256`, `mime`, `bytes` (null when the store doesn't have the file) and `path` (where the file is on this machine). `show` prints the description, then each file with its path. `show` and `export` hash each file again and warn when one is missing or its bytes don't match its name; other commands only check that it's there. `list --search` matches the text and the captions but not link targets — otherwise searching for `media` would match every task with a file.
8. **Export to Notion.** The Description cell is the markdown as stored. The files the exported rows link to are copied into a `media/` folder beside the CSV, where the cell's links resolve. A file already there with the right bytes is left alone; one with other bytes stops the export, because `--out` never overwrites (ADR-007). Notion's importer doesn't upload files, so the report says the files stay on disk to be added by hand. `export --json` adds `media_files`: the files it copied. The "changed since" comparison is untouched — the cell doesn't depend on `--out`, so a new paragraph or a new file shows as a change and nothing else does. If recording the export fails, the CSV and the files this command copied are deleted (ADR-007, amendment 4).

## Alternatives
- **The description as blocks** (the first draft). Deferred: what it adds — merging divergent ledger copies, ids for paragraph-level permissions — waits on sync and permissions, and it costs four event kinds, ordering and tombstone rules, and a forced upgrade. If sync later needs each edit to carry its base, that is a new event kind; `set-body` keeps its shape.
- **Files inside the ledger** (base64). Rejected: every command reads and folds the whole ledger.
- **Only the holder may edit the description.** Rejected: every other piece of metadata is collaborative, and `--base` already prevents lost edits.
- **Merging by line, like git.** Rejected: a paragraph is the unit people rewrite, and the unit a conflict message can name.
- **Recording the original file name.** Rejected: file names carry dates, people and places, the ledger keeps them in cleartext forever, and nothing needs them.
- **Stripping photo metadata.** Deferred: it means rewriting each format's bytes. See Consequences.

## Consequences
- No new event kind, so every older binary still reads every store: it shows the markdown with its `media/` links as text, and its `desc` replaces the whole description, as before. 0.5 warns about the `[media]` section and ignores it. Still released as **0.6.0**: new options, a new JSON field, a new exit code.
- Two new dependencies: `pulldown-cmark` (without its HTML renderer), to find image links the way CommonMark does — code spans, escapes, `<…>` destinations — and `sha2`, for hashing.
- **Files are stored byte for byte**, including any metadata the device wrote — a phone photo can carry the place it was taken. Screenshots and screen recordings carry none. The README says so; stripping is later work.
- The store's own `.gitignore` covers `media/`. A team that commits its store commits its files too, and a 64 MiB video is past the size at which GitHub warns (50 MiB).
- `--base` names a moment by `seq`. Within one ledger file, timestamps strictly increase (store.rs), so fold order is append order and a `seq` always names the same description. Syncing copies of a ledger will need a base that survives a merge.
- Later: cleaning up unreferenced files, audio and PDF, thumbnails, metadata stripping, and uploading files through the Notion API.
