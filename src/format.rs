//! A team's task format (ADR-012): what its tasks should carry — required
//! fields, and description sections with something under them — checked
//! after a write, as warnings. Etiquette, never physics: the fold doesn't
//! read the config, and nothing here refuses a write.

use crate::config::Config;
use crate::model::Task;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Where to start: printed by `hippo-task format` when there's none yet.
pub const EXAMPLE: &str = r#"[format]
guide = """
Titles start with a verb and name the thing: "Fix token refresh on 401".
One outcome per task. If the title needs "and", split it.
"""
template = """
## Why

## Done when
- [ ]
"""
required_fields = ["project"]     # declared under [fields.project]
required_sections = ["Done when"]
"#;

/// What a task misses, one message per gap, each naming the fix. Closed
/// tasks miss nothing: they're history, not work to shape.
pub fn gaps(config: &Config, task: &Task) -> Vec<String> {
    if task.state.is_closed() {
        return Vec::new();
    }
    let format = &config.format;
    let mut gaps = Vec::new();
    for name in &format.required_fields {
        if task.fields.contains_key(name) {
            continue;
        }
        let allowed = config
            .field(name)
            .and_then(|f| f.values.as_ref())
            .map(|values| format!(" (one of: {})", values.join(", ")))
            .unwrap_or_default();
        gaps.push(format!(
            "{} has no {name}, which this project's task format requires — set it: hippo-task update {} --field {name}=<value>{allowed}",
            task.handle(),
            task.num
        ));
    }
    let sections = sections(task.body.as_deref().unwrap_or(""));
    for name in &format.required_sections {
        let key = heading_key(name);
        if sections.iter().any(|s| s.key == key && s.filled) {
            continue;
        }
        gaps.push(format!(
            "{} has nothing under a \"{name}\" heading, which this project's task format requires (see `hippo-task format`) — fill it in: hippo-task desc {} --base {} --file <markdown>",
            task.handle(),
            task.num,
            task.seq
        ));
    }
    gaps
}

/// The headings of a markdown text, as matching keys, in order.
pub fn headings(markdown: &str) -> Vec<String> {
    sections(markdown).into_iter().map(|s| s.key).collect()
}

/// How a heading is compared: case, edge spaces and a trailing colon don't
/// count, so `### done WHEN:` is a "Done when" section.
pub fn heading_key(text: &str) -> String {
    text.trim().trim_end_matches(':').trim_end().to_lowercase()
}

/// One heading, and whether anything sits under it.
struct Section {
    key: String,
    level: usize,
    filled: bool,
}

/// Every heading with whether text sits under it — before the next heading of
/// the same or a higher level, so a subsection's text fills its parent too.
/// An empty checklist item or an HTML comment isn't text: an untouched
/// template fills nothing.
fn sections(markdown: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    // Indexes into `sections` of the headings still open, outermost first.
    let mut open: Vec<usize> = Vec::new();
    let mut heading: Option<(usize, String)> = None;
    // A block's text, judged when the block ends: the parser may hand over
    // `[ ]` as three pieces.
    let mut text = String::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                fill(&mut sections, &open, &mut text);
                heading = Some((level as usize, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = heading.take() {
                    open.retain(|&i| sections[i].level < level);
                    open.push(sections.len());
                    sections.push(Section {
                        key: heading_key(&text),
                        level,
                        filled: false,
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => match &mut heading {
                Some((_, title)) => title.push_str(&t),
                None => text.push_str(&t),
            },
            Event::End(_) => fill(&mut sections, &open, &mut text),
            Event::Start(Tag::Image { .. }) if heading.is_none() => {
                text.push_str("image");
            }
            _ => {}
        }
    }
    fill(&mut sections, &open, &mut text);
    sections
}

/// Mark the open sections filled if `text` says something; then start over.
fn fill(sections: &mut [Section], open: &[usize], text: &mut String) {
    if is_content(text) {
        for &i in open {
            sections[i].filled = true;
        }
    }
    text.clear();
}

/// Text that says something — not blank, and not a bare checkbox (`[ ]`,
/// which is plain text where a list item ends the description).
fn is_content(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty() && !matches!(t, "[ ]" | "[]" | "[x]" | "[X]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(markdown: &str) -> Vec<(String, bool)> {
        sections(markdown)
            .into_iter()
            .map(|s| (s.key, s.filled))
            .collect()
    }

    #[test]
    fn an_untouched_template_fills_nothing() {
        let template = "## Why\n\n## Done when\n- [ ]\n<!-- proof -->\n";
        assert_eq!(
            filled(template),
            [("why".to_string(), false), ("done when".to_string(), false)]
        );
    }

    #[test]
    fn an_empty_checkbox_at_the_very_end_fills_nothing() {
        assert_eq!(
            filled("## Done when\n- [ ]"),
            [("done when".to_string(), false)]
        );
    }

    #[test]
    fn a_checked_or_written_item_fills_its_section() {
        assert_eq!(
            filled("## Done when\n- [ ] the retry happens once"),
            [("done when".to_string(), true)]
        );
    }

    #[test]
    fn subsection_text_fills_the_parent_but_a_sibling_does_not() {
        let md = "# Task\n## Done when\n### Checks\nok\n## Notes\n";
        assert_eq!(
            filled(md),
            [
                ("task".to_string(), true),
                ("done when".to_string(), true),
                ("checks".to_string(), true),
                ("notes".to_string(), false),
            ]
        );
    }

    #[test]
    fn a_hash_inside_code_is_not_a_heading() {
        assert_eq!(headings("```\n# not a heading\n```\n## Real"), ["real"]);
    }

    #[test]
    fn heading_keys_ignore_case_and_a_trailing_colon() {
        assert_eq!(heading_key("  Done WHEN: "), "done when");
    }
}
