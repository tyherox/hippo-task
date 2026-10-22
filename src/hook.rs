//! Session hooks for Claude Code (ADR-014) and Codex (ADR-015): an identity
//! per session, and its claims given back when it ends — opt-in, by adding
//! the hooks to the agent's settings (`init --agents` does it).
//!
//! Both agents run a hook with JSON on stdin that names the session
//! (`session_id`). In Claude Code, a `SessionStart` hook may append `export`
//! lines to the file named by `$CLAUDE_ENV_FILE`, which every later Bash call
//! in that session sources. Codex has no such file: its `SessionStart` output
//! is added to the agent's context, so the hook tells the agent its identity,
//! and the agent puts it in front of each command.
//!
//! The node is drawn from the session id, a random id the agent assigns:
//! nothing about the person or the machine is recorded.

use crate::error::{Error, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Who a Claude Code session acts as, unless the person launched it as someone else.
pub const ACTOR: &str = "agent:claude";
/// Who a Codex session acts as, unless the person launched it as someone else.
pub const CODEX_ACTOR: &str = "agent:codex";

/// What a hook needs from Claude Code's input.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub session_id: String,
    /// The session's folder, when the input names one.
    pub cwd: Option<PathBuf>,
}

/// Read the hook's JSON input. It must name the session.
pub fn parse(stdin: &str) -> Result<Input> {
    let missing = || {
        Error::Usage(
            "expected Claude Code's hook input on stdin: JSON with a session_id — add this command as a SessionStart or SessionEnd hook".into(),
        )
    };
    let value: serde_json::Value = serde_json::from_str(stdin).map_err(|_| missing())?;
    let session_id = value["session_id"]
        .as_str()
        .filter(|id| id.chars().filter(char::is_ascii_alphanumeric).count() >= 4)
        .ok_or_else(missing)?
        .to_string();
    let cwd = value["cwd"].as_str().map(PathBuf::from);
    Ok(Input { session_id, cwd })
}

/// This session's node: `claude-` and the first 8 letters or digits of its id.
pub fn node(input: &Input) -> String {
    let short: String = input
        .session_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect();
    format!("claude-{short}")
}

/// A Codex session's node: `codex-` and the *last* 8 letters or digits of its
/// id. Codex ids are UUIDv7: they start with a clock reading, the same for
/// every session started within a minute or so, and end with random bits.
pub fn codex_node(input: &Input) -> String {
    let id: Vec<char> = input
        .session_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    let tail: String = id.iter().skip(id.len().saturating_sub(8)).collect();
    format!("codex-{tail}")
}

/// The line a Codex session starts with: its identity, and what to put in
/// front of each command — only what the person didn't already launch Codex
/// with (`actor`, `node`). Empty when both are set: every shell has them.
pub fn codex_context(input: &Input, actor: Option<&str>, node: Option<&str>) -> String {
    let mut prefix = Vec::new();
    let actor = actor.unwrap_or_else(|| {
        prefix.push(format!("HIPPO_ACTOR={CODEX_ACTOR}"));
        CODEX_ACTOR
    });
    let node = match node {
        Some(node) => node.to_string(),
        None => {
            let node = codex_node(input);
            prefix.push(format!("HIPPO_NODE={node}"));
            node
        }
    };
    if prefix.is_empty() {
        return String::new();
    }
    format!(
        "hippo-task: this session acts as {actor} on node {node}. Your shell forgets variables between commands, so put {} in front of every hippo-task command. Whatever this node still holds goes back when the session ends.\n",
        prefix.join(" ")
    )
}

/// The `export` lines a session needs: only what the person didn't already
/// set when launching it. Empty when both are set.
pub fn exports(input: &Input, actor_set: bool, node_set: bool) -> String {
    let mut lines = String::new();
    if !actor_set {
        lines.push_str(&format!("export HIPPO_ACTOR='{ACTOR}'\n"));
    }
    if !node_set {
        lines.push_str(&format!("export HIPPO_NODE='{}'\n", node(input)));
    }
    lines
}

/// Append `lines` to the session's env file — never replacing what other
/// hooks wrote there.
pub fn append(env_file: &Path, lines: &str) -> Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env_file)
        .map_err(|e| Error::io(format!("couldn't open {}", env_file.display()), e))?;
    file.write_all(lines.as_bytes())
        .and_then(|()| file.flush())
        .map_err(|e| Error::io(format!("couldn't write {}", env_file.display()), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_node_is_drawn_from_the_session_id() {
        let input = parse(r#"{"session_id":"1a2b3c4d-5e6f-4a7b"}"#).unwrap();
        assert_eq!(node(&input), "claude-1a2b3c4d");
    }

    #[test]
    fn exports_fill_in_only_what_is_missing() {
        let input = parse(r#"{"session_id":"abcd1234"}"#).unwrap();
        assert_eq!(
            exports(&input, false, false),
            "export HIPPO_ACTOR='agent:claude'\nexport HIPPO_NODE='claude-abcd1234'\n"
        );
        assert_eq!(
            exports(&input, true, false),
            "export HIPPO_NODE='claude-abcd1234'\n"
        );
        assert_eq!(exports(&input, true, true), "");
    }

    #[test]
    fn a_codex_node_comes_from_the_random_end_of_its_id() {
        // UUIDv7: the first 8 characters are a clock reading, shared by two
        // sessions started in the same minute.
        let first = parse(r#"{"session_id":"01a1363f-0a73-7180-ad3c-fbdddacabf3f"}"#).unwrap();
        let second = parse(r#"{"session_id":"01a1363f-1b22-7d41-9e01-0c4f5a6b7c8d"}"#).unwrap();
        assert_eq!(codex_node(&first), "codex-dacabf3f");
        assert_eq!(codex_node(&second), "codex-5a6b7c8d");
        let short = parse(r#"{"session_id":"thr_12"}"#).unwrap();
        assert_eq!(codex_node(&short), "codex-thr12");
    }

    #[test]
    fn codex_context_names_only_what_is_missing() {
        let input = parse(r#"{"session_id":"01a1363f-0a73-7180-ad3c-fbdddacabf3f"}"#).unwrap();
        let both = codex_context(&input, None, None);
        assert!(
            both.contains("put HIPPO_ACTOR=agent:codex HIPPO_NODE=codex-dacabf3f in front"),
            "{both}"
        );
        let node_only = codex_context(&input, Some("agent:qa"), None);
        assert!(
            node_only.contains("acts as agent:qa on node codex-dacabf3f"),
            "{node_only}"
        );
        assert!(
            node_only.contains("put HIPPO_NODE=codex-dacabf3f in front"),
            "{node_only}"
        );
        assert_eq!(codex_context(&input, Some("agent:qa"), Some("qa-1")), "");
    }

    #[test]
    fn input_must_name_a_usable_session() {
        for bad in [
            "",
            "{}",
            r#"{"session_id":"!!"}"#,
            r#"{"session_id":7}"#,
            "nope",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
