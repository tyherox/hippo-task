//! Where a project's tasks live (ADR-005): chosen once, found from anywhere.
//!
//! A *store* is a folder holding `ledger.jsonl`. A project finds its store the
//! way git finds `.git`: from the current folder upward to the nearest
//! `.hippotask/`, never above the enclosing repository's root. That
//! `.hippotask/` either **is** the store, or holds a small pointer —
//! `store.json` — to a store that lives somewhere else. The pointer's `kind`
//! is where a hosted store would plug in one day; today only `local` exists.
//!
//! Nothing here creates a store while *looking* for one. `hippo-task init` is
//! how a store comes into being (plus an explicit `--dir`, for scripts), so a
//! typo or an agent working in a subfolder can never split a project's tasks.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// The folder that marks a project — and is usually its store.
pub const DIR: &str = ".hippotask";
/// The pointer inside [`DIR`] when the store lives somewhere else.
pub const POINTER: &str = "store.json";
/// The pointer format this version writes and reads.
const POINTER_VERSION: u32 = 1;
/// Written inside a store to keep it out of git. `*` ignores everything in
/// the folder — this file included — so the repository's own `.gitignore`
/// is never touched.
const IGNORE_ALL: &str = "# Written by `hippo-task init`: keeps this folder out of git.\n*\n";

/// A project's pointer to a store that lives somewhere else.
///
/// Rust note: unknown JSON fields are ignored when reading, so a pointer from
/// a newer hippo-task (say, `"kind": "hosted"` with a `"url"`) still parses —
/// and then gets a clear "upgrade" error instead of a confusing parse error.
#[derive(Debug, Serialize, Deserialize)]
pub struct Pointer {
    pub version: u32,
    /// `"local"` today.
    pub kind: String,
    /// The store folder, for a local store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

// ------------------------------------------------------------------ finding

/// The store for the project containing `start`: the nearest `.hippotask/` in
/// `start` or a folder above it — never climbing out of a git repository.
pub fn discover(start: &Path) -> Result<PathBuf> {
    // Rust note: `ancestors()` yields `start` itself, then each parent up to `/`.
    for dir in start.ancestors() {
        let marker = dir.join(DIR);
        if marker.is_dir() {
            return follow(&marker);
        }
        if dir.join(".git").exists() {
            break; // a repository's root: a store above it belongs to something else
        }
    }
    Err(Error::Usage(
        "no task store here or in any folder above it — a person chooses where this project's tasks live by running `hippo-task init`".into(),
    ))
}

/// `.hippotask/` is the store itself — unless it holds a pointer to one elsewhere.
pub fn follow(marker: &Path) -> Result<PathBuf> {
    let pointer_path = marker.join(POINTER);
    if !pointer_path.exists() {
        return Ok(marker.to_path_buf());
    }
    let text = fs::read_to_string(&pointer_path)
        .map_err(|e| Error::io(format!("couldn't read {}", pointer_path.display()), e))?;
    let pointer: Pointer = serde_json::from_str(&text).map_err(|e| {
        Error::Usage(format!(
            "{} is unreadable ({e}) — run `hippo-task init` to choose again",
            pointer_path.display()
        ))
    })?;
    let this = env!("CARGO_PKG_VERSION");
    if pointer.version > POINTER_VERSION {
        return Err(Error::Usage(format!(
            "{} was written by a newer hippo-task — upgrade hippo-task (this is {this})",
            pointer_path.display()
        )));
    }
    match (pointer.kind.as_str(), pointer.path) {
        ("local", Some(path)) if path.is_dir() => Ok(path),
        ("local", Some(path)) => Err(Error::Usage(format!(
            "this project's tasks live in {}, which doesn't exist any more — restore that folder, or run `hippo-task init` to choose again",
            path.display()
        ))),
        ("local", None) => Err(Error::Usage(format!(
            "{} names no folder — run `hippo-task init` to choose again",
            pointer_path.display()
        ))),
        (kind, _) => Err(Error::Usage(format!(
            "this project's tasks live in a {kind} store, which hippo-task {this} can't open — upgrade hippo-task"
        ))),
    }
}

/// The root of the git repository containing `path`, if any: the nearest
/// folder with a `.git` (a folder, or the file a worktree has). Works for a
/// path that doesn't exist yet, since only its existing parents can match.
pub fn git_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// The folder `init` sets up: the enclosing repository's root, else `start`.
pub fn project_root(start: &Path) -> PathBuf {
    git_root(start).unwrap_or_else(|| start.to_path_buf())
}

// --------------------------------------------------------------- setting up

/// Where `init` puts a project's tasks.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// `<project>/.hippotask/` — the default.
    Here,
    /// Another folder (an absolute path); the project gets a pointer to it.
    Folder(PathBuf),
}

/// What a person chose.
#[derive(Debug, Clone)]
pub struct Choice {
    pub place: Place,
    /// Inside a git repository: keep the tasks out of git (the default).
    pub keep_out_of_git: bool,
}

/// What `init` did (or found).
#[derive(Debug)]
pub struct Setup {
    pub project: PathBuf,
    /// The store folder.
    pub store: PathBuf,
    /// True when the store lives elsewhere and the project holds a pointer.
    pub pointer: bool,
    /// False when the project already had a store — `init` never moves one.
    pub created: bool,
    /// The git repository the store sits in, if any.
    pub repository: Option<PathBuf>,
    /// Whether the store's own `.gitignore` keeps it out of git.
    pub kept_out_of_git: bool,
}

/// Set a project up as chosen. Safe to run again: it then reports the store the
/// project already has, adds the git protection if asked — and never moves a
/// ledger.
pub fn apply(project: &Path, choice: &Choice) -> Result<Setup> {
    let project = canonical(project)?;
    let marker = project.join(DIR);
    let existed = marker.is_dir();
    let store = if existed {
        let current = canonical(&follow(&marker)?)?;
        if let Place::Folder(wanted) = &choice.place {
            if canonical(wanted).ok().as_deref() != Some(current.as_path()) {
                return Err(Error::Usage(format!(
                    "this project already keeps its tasks in {} — moving them isn't supported yet",
                    current.display()
                )));
            }
        }
        current
    } else {
        match &choice.place {
            Place::Here => {
                create(&marker)?;
                canonical(&marker)?
            }
            Place::Folder(folder) => {
                if !folder.is_absolute() {
                    return Err(Error::Usage(format!(
                        "{} isn't an absolute path",
                        folder.display()
                    )));
                }
                create(folder)?;
                let folder = canonical(folder)?;
                create(&marker)?;
                write_pointer(&marker, &folder)?;
                // The pointer names a path on this machine: it's never committed.
                ignore_all(&marker)?;
                folder
            }
        }
    };
    let repository = git_root(&store);
    if repository.is_some() && choice.keep_out_of_git {
        ignore_all(&store)?;
    }
    Ok(Setup {
        pointer: marker.join(POINTER).exists(),
        kept_out_of_git: store.join(".gitignore").exists(),
        project,
        store,
        created: !existed,
        repository,
    })
}

/// Ask a person where this project's tasks should live. Questions go to `out`
/// and answers come from `input`: the CLI passes the terminal, tests pass
/// buffers. An empty answer (or end of input) takes the default.
pub fn ask(project: &Path, input: &mut impl BufRead, out: &mut impl Write) -> Result<Choice> {
    say(
        out,
        &format!(
            "Setting up hippo-task for {}\n\n\
             Where should this project's tasks live?\n  \
             1) In this project, in {DIR}/   (recommended)\n  \
             2) In another folder, outside this project\n\
             Choose 1 or 2 [1]: ",
            project.display()
        ),
    )?;
    let place = match answer(input)?.as_str() {
        "" | "1" => Place::Here,
        "2" => {
            say(out, "Folder for this project's tasks: ")?;
            let typed = answer(input)?;
            if typed.is_empty() {
                return Err(Error::Usage("no folder given".into()));
            }
            Place::Folder(absolute(&typed)?)
        }
        other => {
            return Err(Error::Usage(format!(
                "'{other}' isn't one of the choices — answer 1 or 2"
            )))
        }
    };
    let store = match &place {
        Place::Here => project.join(DIR),
        Place::Folder(folder) => folder.clone(),
    };
    // Only a store inside a repository has anything to decide about git.
    let keep_out_of_git = match git_root(&store) {
        None => true,
        Some(root) => {
            say(
                out,
                &format!(
                    "\n{} is inside a git repository ({}).\n\
                     Tasks are plain text and permanent, and `git add -A` would commit every title and note.\n\
                     Keep them out of git? [Y/n]: ",
                    store.display(),
                    root.display()
                ),
            )?;
            match answer(input)?.to_ascii_lowercase().as_str() {
                "" | "y" | "yes" => true,
                "n" | "no" => false,
                other => return Err(Error::Usage(format!("'{other}' isn't an answer — y or n"))),
            }
        }
    };
    Ok(Choice {
        place,
        keep_out_of_git,
    })
}

/// A typed path made absolute: `~/…` means the home folder (so does `~\…`, on
/// Windows), and a relative path is taken from the current folder.
///
/// Rust note: std's `home_dir` is `$HOME` on macOS and Linux, and the user's
/// profile folder on Windows, where `HOME` usually isn't set at all.
pub fn absolute(typed: &str) -> Result<PathBuf> {
    let rest = typed.strip_prefix("~/").or_else(|| {
        if cfg!(windows) {
            typed.strip_prefix("~\\")
        } else {
            None
        }
    });
    let expanded = match (rest, std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(typed),
    };
    std::path::absolute(&expanded).map_err(|e| Error::io(format!("couldn't resolve {typed}"), e))
}

// -------------------------------------------------------------------- guide

/// The agent protocol: §A of AGENTS.md, built into the binary so it always
/// matches the commands it describes — a project's AGENTS.md only has to say
/// "run `hippo-task guide`".
pub fn guide() -> String {
    const AGENTS: &str = include_str!("../AGENTS.md");
    // Byte offsets of the two headings; `\n` is ASCII, so both are char boundaries.
    let start = AGENTS.find("\n## A. ").map(|at| at + 1);
    let end = AGENTS.find("\n## B. ");
    match (start, end) {
        (Some(start), Some(end)) if start < end => AGENTS[start..end].trim_end().to_string(),
        _ => AGENTS.trim_end().to_string(),
    }
}

// ------------------------------------------------------------------ helpers

fn say(out: &mut impl Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| Error::io("couldn't write the question", e))
}

fn answer(input: &mut impl BufRead) -> Result<String> {
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|e| Error::io("couldn't read the answer", e))?;
    Ok(line.trim().to_string())
}

fn create(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| Error::io(format!("couldn't create {}", dir.display()), e))
}

/// The real, absolute path: symlinks resolved, `..` gone — the form `init`
/// stores and shows, so the same folder always reads the same.
fn canonical(path: &Path) -> Result<PathBuf> {
    let real = fs::canonicalize(path)
        .map_err(|e| Error::io(format!("couldn't resolve {}", path.display()), e))?;
    Ok(plain(real))
}

/// On Windows, `canonicalize` returns a "verbatim" path (`\\?\C:\work`), a
/// form people don't type and some tools don't accept. For an ordinary drive
/// path, the plain form (`C:\work`) means the same folder, so use that.
/// Elsewhere, paths pass through unchanged.
fn plain(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(rest) = path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        // `C:\…` has a drive letter, then a colon. (`\\?\UNC\…` — a network
        // share — is left as it is.)
        if rest.as_bytes().get(1) == Some(&b':') {
            return PathBuf::from(rest);
        }
    }
    path
}

/// Keep `dir` out of git with its own `.gitignore` — unless it already has one.
fn ignore_all(dir: &Path) -> Result<()> {
    let file = dir.join(".gitignore");
    if file.exists() {
        return Ok(());
    }
    fs::write(&file, IGNORE_ALL)
        .map_err(|e| Error::io(format!("couldn't write {}", file.display()), e))
}

fn write_pointer(marker: &Path, folder: &Path) -> Result<()> {
    let pointer = Pointer {
        version: POINTER_VERSION,
        kind: "local".into(),
        path: Some(folder.to_path_buf()),
    };
    let file = marker.join(POINTER);
    let text = serde_json::to_string_pretty(&pointer)
        .map_err(|e| Error::Usage(format!("couldn't encode the pointer: {e}")))?;
    fs::write(&file, text + "\n")
        .map_err(|e| Error::io(format!("couldn't write {}", file.display()), e))
}
