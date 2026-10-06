//! Render draft descriptions without allowing Markdown to introduce executable
//! HTML, arbitrary URLs, or automatic requests outside the local store.

use crate::media;
use pulldown_cmark::{html, Event, LinkType, Options, Parser, Tag, TagEnd};

pub(super) fn render(source: &str) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut parser = Parser::new_ext(source, options);
    let mut safe = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            // Raw HTML is visible source, never markup. All other source text
            // is escaped by pulldown-cmark's HTML renderer too.
            Event::Html(text) | Event::InlineHtml(text) => safe.push(Event::Text(text)),
            Event::Start(Tag::Link {
                dest_url,
                title,
                link_type,
                ..
            }) => {
                let url = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.to_string()
                };
                let opening = if allowed_link(&url) {
                    format!("<a href=\"{}\" title=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">", escape(&url), escape(&title))
                } else {
                    // Use the same closing tag for both cases; an anchor
                    // without href retains its readable text but cannot act.
                    "<a>".into()
                };
                safe.push(Event::InlineHtml(opening.into()));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                // Flatten the complete image subtree into an escaped caption.
                // Nested image/link markup must never leak into an attribute.
                let mut caption = String::new();
                let mut depth = 1;
                for part in parser.by_ref() {
                    match part {
                        Event::Start(Tag::Image { .. }) => depth += 1,
                        Event::End(TagEnd::Image) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Event::Text(text) | Event::Code(text) => caption.push_str(&text),
                        Event::SoftBreak | Event::HardBreak => caption.push(' '),
                        _ => {}
                    }
                }
                let caption = escape(&caption);
                let placeholder = if let Some(name) = media::store_name(&dest_url) {
                    format!("<span class=\"preview-media\" data-media=\"{name}\" data-caption=\"{caption}\">{caption}<small>Loading media…</small></span>")
                } else {
                    format!("<span class=\"preview-media unavailable\">{caption}<small>External image not loaded</small></span>")
                };
                safe.push(Event::InlineHtml(placeholder.into()));
            }
            event => safe.push(event),
        }
    }
    let mut output = String::new();
    html::push_html(&mut output, safe.into_iter());
    output
}

fn allowed_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    !url.chars().any(char::is_control)
        && (lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
