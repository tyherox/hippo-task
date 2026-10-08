//! Similar tasks (ADR-013): rank tasks by the distinctive words they share
//! with a text, so an agent reads the likely originals before filing.
//!
//! A ranking, not a verdict. Replayed over the pilot's backlog, the shared
//! words weighted by rarity put every known duplicate's original first —
//! while any threshold for *warning* either nagged or missed (ADR-004). So
//! nothing here decides "duplicate": the agent reads the top few and judges.

use crate::media;
use crate::model::{RelType, Task};
use std::collections::{BTreeSet, HashMap};

/// Words too common in task titles to say anything on their own.
const STOP: [&str; 34] = [
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "does", "for", "from", "has", "in",
    "into", "is", "it", "its", "no", "not", "of", "on", "or", "so", "than", "that", "the", "then",
    "this", "to", "when", "with", "after", "before",
];

/// The tasks sharing distinctive words with `text`, best first, at most
/// `limit`. Tasks marked `duplicate-of` are skipped, so their original
/// surfaces instead. Ties go to the older task: the original came first.
pub fn rank<'a>(tasks: &'a [Task], text: &str, limit: usize) -> Vec<&'a Task> {
    let query = words(text);
    let docs: Vec<(&Task, BTreeSet<String>)> = tasks
        .iter()
        .filter(|t| !is_marked_duplicate(t))
        .map(|t| (t, task_words(t)))
        .collect();
    // How many tasks use each word: the rarer, the more a shared one says.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for (_, ws) in &docs {
        for w in ws {
            *df.entry(w.as_str()).or_default() += 1;
        }
    }
    let n = docs.len() as f64;
    let mut scored: Vec<(f64, &Task)> = docs
        .iter()
        .filter_map(|(t, ws)| {
            let score: f64 = query
                .iter()
                .filter(|w| ws.contains(*w))
                .map(|w| {
                    let used = df.get(w.as_str()).copied().unwrap_or(0) as f64;
                    ((n + 1.0) / (used + 0.5)).ln()
                })
                .sum();
            (score > 0.0).then_some((score, *t))
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.num.cmp(&b.1.num)));
    scored.into_iter().take(limit).map(|(_, t)| t).collect()
}

/// A title reduced to its words, for spotting an identical one: case,
/// punctuation and spacing don't count. "Fix the Token-Refresh  bug!" and
/// "fix the token refresh bug" are the same.
pub fn same_title_key(title: &str) -> String {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_marked_duplicate(t: &Task) -> bool {
    t.relations.iter().any(|r| r.rel == RelType::DuplicateOf)
}

/// A task's words: its title, and its description's prose and captions —
/// not its image paths, which would make `media` match every screenshot.
fn task_words(t: &Task) -> BTreeSet<String> {
    let mut ws = words(&t.title);
    if let Some(body) = &t.body {
        ws.extend(words(&media::searchable(body)));
    }
    ws
}

/// The distinctive words of `text`: lowercase runs of letters, digits, and
/// `_ . / -`, so identifiers like `session_token` and `settings.yaml` stay
/// whole; trimmed of edge punctuation, without one-letter and stop words.
fn words(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || "_./-".contains(c)))
        .map(|w| w.trim_matches(|c: char| "_./-".contains(c)))
        .filter(|w| w.chars().count() > 1 && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_identifiers_whole_and_drop_noise() {
        let ws = words("Fix session_token in settings.yaml: a 401, (again).");
        let ws: Vec<&str> = ws.iter().map(String::as_str).collect();
        assert_eq!(
            ws,
            ["401", "again", "fix", "session_token", "settings.yaml"]
        );
    }

    #[test]
    fn words_keep_non_latin_text() {
        let ws = words("토큰 갱신 실패");
        assert_eq!(ws.len(), 3);
    }

    #[test]
    fn same_title_key_ignores_case_punctuation_and_spacing() {
        assert_eq!(
            same_title_key("Fix the Token-Refresh  bug!"),
            same_title_key("fix the token refresh bug")
        );
        assert_ne!(same_title_key("Fix a"), same_title_key("Fix b"));
    }
}
