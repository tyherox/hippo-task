//! Setting up a project's agents (ADR-015): what `init` does when asked. Each
//! agent gets the skill and its session hooks, merged into its own settings,
//! and the project's instructions get one line. Every write keeps what's
//! already there, and running it again changes nothing.

use crate::error::{Error, Result};
use crate::setup;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// An agent `init` can set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    /// The name `--agents` and `init --json` use.
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    /// The name people know it by.
    pub fn title(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    /// Where this agent reads a project's skills, from the project's root.
    pub fn skills(self) -> &'static str {
        match self {
            Agent::Claude => ".claude/skills",
            Agent::Codex => ".agents/skills",
        }
    }

    /// The file holding this agent's hooks, from the project's root.
    pub fn hooks_file(self) -> &'static str {
        match self {
            Agent::Claude => ".claude/settings.json",
            Agent::Codex => ".codex/hooks.json",
        }
    }

    /// The command its hook runs for `event`: `session-start` or `session-end`.
    pub fn command(self, event: &str) -> String {
        match self {
            Agent::Claude => format!("hippo-task hook {event}"),
            Agent::Codex => format!("hippo-task hook {event} --agent codex"),
        }
    }
}

/// The line `init` adds to a project's instructions, for agents that read
/// no skills.
pub const LINE: &str =
    "Tasks are coordinated with hippo-task: use its skill, or run `hippo-task guide` if you have none.";

/// The hook events `init` adds, with the `hippo-task hook` event each runs.
const EVENTS: [(&str, &str); 2] = [
    ("SessionStart", "session-start"),
    ("SessionEnd", "session-end"),
];

/// What `set_up` did for one agent.
#[derive(Debug)]
pub struct Connected {
    pub agent: Agent,
    /// The skill's folder.
    pub skill: PathBuf,
    /// The agent's hooks file.
    pub hooks: PathBuf,
    /// True when `set_up` added hooks to it.
    pub hooks_added: bool,
    /// True when the file couldn't be read safely, so it was left as it was
    /// (the report's `warnings` say why).
    pub hooks_left_alone: bool,
}

/// The project's instructions file, and whether `set_up` added the line.
#[derive(Debug)]
pub struct Instructions {
    pub file: PathBuf,
    pub added: bool,
}

/// What `set_up` did.
#[derive(Debug, Default)]
pub struct Report {
    pub agents: Vec<Connected>,
    /// `None` when no agent was set up.
    pub instructions: Option<Instructions>,
    /// A hooks file left as it was, why, and what to add by hand.
    pub warnings: Vec<String>,
}

/// Set up `agents` for the project at `project`.
pub fn set_up(project: &Path, agents: &[Agent]) -> Result<Report> {
    let mut report = Report::default();
    let mut done: Vec<Agent> = Vec::new();
    for &agent in agents {
        if done.contains(&agent) {
            continue;
        }
        done.push(agent);
        let (skill, _) = setup::write_skill(&join(project, agent.skills()))?;
        let hooks = join(project, agent.hooks_file());
        let (hooks_added, hooks_left_alone) = match add_hooks(&hooks, agent)? {
            Ok(added) => (added, false),
            Err(why) => {
                report.warnings.push(format!(
                    "left {} as it was: {why}. To give each {} session its own identity, add these hooks to it yourself: {}",
                    hooks.display(),
                    agent.title(),
                    snippet(agent)?
                ));
                (false, true)
            }
        };
        report.agents.push(Connected {
            agent,
            skill,
            hooks,
            hooks_added,
            hooks_left_alone,
        });
    }
    if !done.is_empty() {
        report.instructions = Some(instruct(project, &done)?);
    }
    Ok(report)
}

/// Ask a person which agents work in the project. An empty answer (or end of
/// input) takes the default: both.
pub fn ask(input: &mut impl BufRead, out: &mut impl Write) -> Result<Vec<Agent>> {
    setup::say(
        out,
        "\nWhich agents work in this project? Each gets the hippo-task skill, and hooks that give\n\
         every session its own identity and hand back its tasks when it ends.\n  \
         1) Claude Code and Codex   (recommended)\n  \
         2) Claude Code\n  \
         3) Codex\n  \
         4) Neither: I'll set them up myself\n\
         Choose 1-4 [1]: ",
    )?;
    match setup::answer(input)?.as_str() {
        "" | "1" => Ok(vec![Agent::Claude, Agent::Codex]),
        "2" => Ok(vec![Agent::Claude]),
        "3" => Ok(vec![Agent::Codex]),
        "4" => Ok(Vec::new()),
        other => Err(Error::Usage(format!(
            "'{other}' isn't one of the choices — answer 1, 2, 3 or 4"
        ))),
    }
}

// ------------------------------------------------------------------ hooks

/// Merge `agent`'s hooks into `file`, creating it if needed. The inner result
/// is `Ok(added)`, or `Err(why)` when the file isn't something `init` can
/// change safely — then it's left as it was.
fn add_hooks(file: &Path, agent: Agent) -> Result<std::result::Result<bool, String>> {
    let text = match fs::read_to_string(file) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(Error::io(format!("couldn't read {}", file.display()), e)),
    };
    let merged = match merge_hooks(text.as_deref(), agent) {
        Ok(Some(merged)) => merged,
        Ok(None) => return Ok(Ok(false)),
        Err(why) => return Ok(Err(why)),
    };
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::io(format!("couldn't create {}", parent.display()), e))?;
    }
    fs::write(file, merged)
        .map_err(|e| Error::io(format!("couldn't write {}", file.display()), e))?;
    Ok(Ok(true))
}

/// The hooks file `text` (`None`: there is none) with `agent`'s hooks added:
/// `Ok(None)` when every one is already there. Keys stay in file order, and
/// every value this doesn't change is written back byte for byte.
pub fn merge_hooks(
    text: Option<&str>,
    agent: Agent,
) -> std::result::Result<Option<String>, String> {
    let top = match text.filter(|t| !t.trim().is_empty()) {
        Some(text) => object(text).ok_or("it isn't a JSON object")?,
        None => Vec::new(),
    };
    let old_hooks = match top.iter().find(|(key, _)| key == "hooks") {
        Some((_, raw)) => object(raw.get()).ok_or("its \"hooks\" isn't a JSON object")?,
        None => Vec::new(),
    };

    let mut changed = false;
    let mut hooks: Vec<(String, Json)> = Vec::new();
    for (key, raw) in old_hooks {
        let Some((_, event)) = EVENTS.iter().find(|(name, _)| *name == key) else {
            hooks.push((key, Json::Raw(raw)));
            continue;
        };
        let groups: Vec<Box<RawValue>> = serde_json::from_str(raw.get())
            .map_err(|_| format!("its \"hooks\".\"{key}\" isn't a list"))?;
        let command = agent.command(event);
        if groups.iter().any(|group| runs(group, &command)) {
            hooks.push((key, Json::Raw(raw)));
        } else {
            changed = true;
            let mut kept: Vec<Json> = groups.into_iter().map(Json::Raw).collect();
            kept.push(group(agent, event));
            hooks.push((key, Json::Array(kept)));
        }
    }
    for (name, event) in EVENTS {
        if !hooks.iter().any(|(key, _)| key == name) {
            changed = true;
            hooks.push((name.to_string(), Json::Array(vec![group(agent, event)])));
        }
    }
    if !changed {
        return Ok(None);
    }

    let mut hooks = Some(Json::Object(hooks));
    let mut entries: Vec<(String, Json)> = Vec::new();
    for (key, raw) in top {
        match hooks.take_if(|_| key == "hooks") {
            Some(merged) => entries.push((key, merged)),
            None => entries.push((key, Json::Raw(raw))),
        }
    }
    if let Some(merged) = hooks {
        entries.push(("hooks".to_string(), merged));
    }
    let mut merged = serde_json::to_string_pretty(&Json::Object(entries))
        .map_err(|e| format!("it couldn't be written back ({e})"))?;
    merged.push('\n');
    Ok(Some(merged))
}

/// Does this matcher group already run `command` — perhaps at a full path?
fn runs(group: &RawValue, command: &str) -> bool {
    let group: serde_json::Value = serde_json::from_str(group.get()).unwrap_or_default();
    group["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|hook| {
            hook["command"]
                .as_str()
                .is_some_and(|c| c.contains(command))
        })
    })
}

/// One matcher group running `agent`'s hook for `event`.
fn group(agent: Agent, event: &str) -> Json {
    let text = |s: &str| Json::Value(serde_json::Value::from(s));
    let mut hook = vec![
        ("type".to_string(), text("command")),
        ("command".to_string(), text(&agent.command(event))),
    ];
    // Codex gives an end hook 1 second unless it asks for more (3 at most).
    if agent == Agent::Codex && event == "session-end" {
        hook.push(("timeout".to_string(), Json::Value(3.into())));
    }
    Json::Object(vec![(
        "hooks".to_string(),
        Json::Array(vec![Json::Object(hook)]),
    )])
}

/// `agent`'s hooks on one line, for a person to add by hand.
fn snippet(agent: Agent) -> Result<String> {
    let events = EVENTS
        .iter()
        .map(|(name, event)| (name.to_string(), Json::Array(vec![group(agent, event)])))
        .collect();
    let hooks = Json::Object(vec![("hooks".to_string(), Json::Object(events))]);
    serde_json::to_string(&hooks)
        .map_err(|e| Error::Usage(format!("couldn't encode the hooks: {e}")))
}

/// A JSON value being written back: untouched parts stay raw text.
enum Json {
    Raw(Box<RawValue>),
    Value(serde_json::Value),
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Json::Raw(raw) => raw.serialize(s),
            Json::Value(value) => value.serialize(s),
            Json::Object(entries) => {
                let mut map = s.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
            Json::Array(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
        }
    }
}

/// A JSON object's entries in file order, each value as its raw text — or
/// `None` when `text` isn't an object.
fn object(text: &str) -> Option<Vec<(String, Box<RawValue>)>> {
    serde_json::from_str::<Entries>(text).ok().map(|e| e.0)
}

/// Rust note: serde_json's own `Map` sorts its keys, so reading a settings
/// file into it would reorder the person's file. This reads an object's
/// entries as a list instead.
struct Entries(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct InOrder;
        impl<'de> Visitor<'de> for InOrder {
            type Value = Entries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Entries, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, Box<RawValue>>()? {
                    entries.push(entry);
                }
                Ok(Entries(entries))
            }
        }
        d.deserialize_map(InOrder)
    }
}

// ----------------------------------------------------------- instructions

/// Add [`LINE`] to the project's instructions: AGENTS.md, else CLAUDE.md if
/// that's the one there is; with neither, a new AGENTS.md — or CLAUDE.md
/// when Claude Code is the only agent. A file that already mentions
/// hippo-task is left alone. The line is appended: nothing is rewritten.
fn instruct(project: &Path, agents: &[Agent]) -> Result<Instructions> {
    let file = ["AGENTS.md", "CLAUDE.md"]
        .into_iter()
        .map(|name| project.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            project.join(if agents == [Agent::Claude] {
                "CLAUDE.md"
            } else {
                "AGENTS.md"
            })
        });
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(format!("couldn't read {}", file.display()), e)),
    };
    if text.contains("hippo-task") {
        return Ok(Instructions { file, added: false });
    }
    // Its own paragraph: a blank line after whatever is there.
    let mut addition = String::new();
    if !text.is_empty() {
        let ending = text.len() - text.trim_end_matches('\n').len();
        addition.push_str(&"\n".repeat(2usize.saturating_sub(ending)));
    }
    addition.push_str(LINE);
    addition.push('\n');
    let mut handle = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| Error::io(format!("couldn't open {}", file.display()), e))?;
    handle
        .write_all(addition.as_bytes())
        .and_then(|()| handle.flush())
        .map_err(|e| Error::io(format!("couldn't write {}", file.display()), e))?;
    Ok(Instructions { file, added: true })
}

/// `project` joined with a `/`-separated relative path, one component at a
/// time: on Windows, joining it whole would leave a `/` in the path.
fn join(project: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(project.to_path_buf(), |path, part| path.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hooks_file_holds_just_the_two_hooks() {
        let text = merge_hooks(None, Agent::Codex).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"hooks": {
                "SessionStart": [{"hooks": [{"type": "command", "command": "hippo-task hook session-start --agent codex"}]}],
                "SessionEnd": [{"hooks": [{"type": "command", "command": "hippo-task hook session-end --agent codex", "timeout": 3}]}]
            }})
        );
        assert!(text.ends_with("}\n"), "{text}");
        assert_eq!(merge_hooks(Some(&text), Agent::Codex), Ok(None));
        assert_eq!(merge_hooks(Some(" \n"), Agent::Codex), Ok(Some(text)));
    }

    #[test]
    fn the_snippet_is_what_a_new_file_would_hold() {
        let snippet: serde_json::Value =
            serde_json::from_str(&snippet(Agent::Claude).unwrap()).unwrap();
        let file: serde_json::Value =
            serde_json::from_str(&merge_hooks(None, Agent::Claude).unwrap().unwrap()).unwrap();
        assert_eq!(snippet, file);
    }

    #[test]
    fn a_duplicated_agent_is_set_up_once() {
        let dir = std::env::temp_dir().join(format!("hippo-agents-dup-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let report = set_up(&dir, &[Agent::Claude, Agent::Claude]).unwrap();
        assert_eq!(report.agents.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
