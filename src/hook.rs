//! Claude Code hooks (ADR-014): an identity per session, and its claims given
//! back when it ends — opt-in, by adding the hooks to a settings file.
//!
//! Claude Code runs a hook with JSON on stdin that names the session
//! (`session_id`). A `SessionStart` hook may append `export` lines to the file
//! named by `$CLAUDE_ENV_FILE`, which every later Bash call in that session
//! sources — the one way an identity lasts from one command to the next.
//!
//! The node is drawn from the session id, a random id Claude Code assigns:
//! nothing about the person or the machine is recorded.

use crate::error::{Error, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Who a Claude Code session acts as, unless the person launched it as someone else.
pub const ACTOR: &str = "agent:claude";

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
