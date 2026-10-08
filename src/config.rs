//! A store's settings: `config.toml`, next to `ledger.jsonl` (ADR-006).
//!
//! Today it declares **fields** — named attributes with one value per task,
//! optionally limited to a list of allowed values:
//!
//! ```toml
//! [fields.project]
//! values = ["dashboard", "billing"]
//!
//! [fields.customer]        # no list: any text
//! display_name = "Client"
//! ```
//!
//! It may also declare a **format** (ADR-012): a guide for whoever writes
//! tasks, a description template, and the fields and sections every task
//! should have — see [`crate::format::EXAMPLE`].
//!
//! Only the CLI reads this file — it's etiquette, not physics: the fold accepts
//! whatever the ledger says, so editing the config never changes a task.

use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The file's name inside a store folder.
pub const FILE: &str = "config.toml";

/// Where to start: printed by `hippo-task fields` when there's no config yet.
pub const EXAMPLE: &str = r#"[fields.project]
values = ["dashboard", "billing"]

[fields.team]
values = ["engineering", "design"]

[fields.customer]   # no list: any text
display_name = "Client"
"#;

/// The export's own column headings (ADR-007), before and after the fields'.
/// A field's `display_name` may not repeat one of these, ignoring case.
pub const EXPORT_COLUMNS_BEFORE: [&str; 3] = ["Name", "Status", "Priority"];
pub const EXPORT_COLUMNS_AFTER: [&str; 5] = [
    "Tags",
    "Assignee",
    "Description",
    "Blocked by",
    "hippo-task ID",
];

/// The top-level settings this version reads; any other is ignored, with a warning.
const KNOWN_SETTINGS: [&str; 3] = ["fields", "format", "media"];

/// The settings `[format]` reads; any other is ignored, with a warning — a
/// later version may add some (ADR-012).
const FORMAT_SETTINGS: [&str; 4] = ["guide", "template", "required_fields", "required_sections"];

/// Default cap for one image or GIF, in MiB (ADR-008).
pub const DEFAULT_MAX_IMAGE_MIB: u64 = 8;
/// Default cap for one video, in MiB (ADR-008).
pub const DEFAULT_MAX_VIDEO_MIB: u64 = 64;

/// How large a file copied into `media/` may be. Missing keys use the defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLimits {
    pub max_image_bytes: u64,
    pub max_video_bytes: u64,
}

impl Default for MediaLimits {
    fn default() -> Self {
        MediaLimits {
            max_image_bytes: DEFAULT_MAX_IMAGE_MIB * 1024 * 1024,
            max_video_bytes: DEFAULT_MAX_VIDEO_MIB * 1024 * 1024,
        }
    }
}

/// A store's settings, read from `<store>/config.toml`.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Where the file is — or would be, if it doesn't exist yet.
    pub path: PathBuf,
    /// Whether it exists. A missing file declares nothing.
    pub exists: bool,
    /// The declared fields, sorted by name.
    pub fields: Vec<Field>,
    /// Caps for files copied into `media/` (ADR-008). Defaults when unset.
    pub media: MediaLimits,
    /// What a task should look like here (ADR-012). Empty when undeclared.
    pub format: Format,
    /// Settings this version doesn't know, one message each.
    pub warnings: Vec<String>,
}

/// `[format]`: the team's conventions for writing tasks (ADR-012).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Format {
    /// Prose for whoever writes tasks.
    pub guide: Option<String>,
    /// The description to start from (markdown).
    pub template: Option<String>,
    /// Declared fields every open task should carry.
    pub required_fields: Vec<String>,
    /// Headings every open task's description should have text under.
    pub required_sections: Vec<String>,
}

/// One declared field.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// The id used everywhere else: `--field project=…`, JSON, the ledger.
    pub name: String,
    /// What people see — a column heading in exports.
    pub display_name: String,
    /// The allowed values, in the order the file lists them; `None` = any text.
    pub values: Option<Vec<String>>,
}

impl Field {
    /// Was `display_name` set in the file, rather than derived from the name?
    pub fn has_custom_display_name(&self) -> bool {
        self.display_name != default_display_name(&self.name)
    }
}

// ------------------------------------------------------------- the file

/// The file as written. Top-level settings this version doesn't know land in
/// `other`, to be warned about: a newer hippo-task may add sections (ADR-006).
#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    fields: BTreeMap<String, RawField>,
    #[serde(default)]
    media: RawMedia,
    #[serde(default)]
    format: RawFormat,
    #[serde(flatten)]
    other: BTreeMap<String, toml::Value>,
}

/// `[format]` as written. Unknown keys land in `other`, to be warned about:
/// unlike a field's table, later versions are expected to add settings here.
#[derive(Deserialize, Default)]
struct RawFormat {
    guide: Option<String>,
    template: Option<String>,
    #[serde(default)]
    required_fields: Vec<String>,
    #[serde(default)]
    required_sections: Vec<String>,
    #[serde(flatten)]
    other: BTreeMap<String, toml::Value>,
}

/// `[media]` in config.toml. Unknown keys are an error — a typo here would
/// silently leave the default cap in place.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawMedia {
    max_image_mib: Option<u64>,
    max_video_mib: Option<u64>,
}

/// One field's table. Rust note: `deny_unknown_fields` turns a misspelled
/// setting (`vaules = …`) into an error instead of silently ignoring it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawField {
    values: Option<Vec<String>>,
    display_name: Option<String>,
}

impl Config {
    /// Read `<folder>/config.toml`. A missing file is an empty config — every
    /// store works without one.
    pub fn load(folder: &Path) -> Result<Config> {
        let path = folder.join(FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Ok(Config {
                    path,
                    ..Config::default()
                })
            }
            Err(e) => return Err(Error::io(format!("couldn't read {}", path.display()), e)),
        };
        let raw: RawConfig = toml::from_str(&text).map_err(|e| {
            Error::Usage(format!(
                "{} isn't valid: {}",
                path.display(),
                e.to_string().trim_end()
            ))
        })?;
        let fields = raw
            .fields
            .into_iter()
            .map(|(name, raw)| field(name, raw))
            .collect::<std::result::Result<Vec<_>, String>>()
            .and_then(|fields| unique_headings(&fields).map(|()| fields))
            .map_err(|why| Error::Usage(format!("{}: {why}", path.display())))?;
        let media = media_limits(&path, &raw.media)?;
        let format = format(&raw.format, &fields)
            .map_err(|why| Error::Usage(format!("{}: [format] {why}", path.display())))?;
        let mut warnings = unknown(&path, "", raw.other.keys(), &KNOWN_SETTINGS);
        warnings.extend(unknown(
            &path,
            "[format] ",
            raw.format.other.keys(),
            &FORMAT_SETTINGS,
        ));
        Ok(Config {
            path,
            exists: true,
            fields,
            media,
            format,
            warnings,
        })
    }

    /// The declared field called `name`, if there is one.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The field to *set*, checked: it must be declared, and the value must be
    /// on its list (if it has one). Errors suggest the closest match.
    pub fn check(&self, name: &str, value: &str) -> Result<&Field> {
        let field = self.declared(name)?;
        if let Some(values) = &field.values {
            if !values.iter().any(|v| v == value) {
                let hint = closest(value, values)
                    .map(|v| format!(" — did you mean `{v}`?"))
                    .unwrap_or_else(|| ".".to_string());
                return Err(Error::Usage(format!(
                    "`{value}` isn't an allowed value for {name}{hint} Allowed: {} (see `hippo-task fields`)",
                    values.join(", ")
                )));
            }
        }
        Ok(field)
    }

    /// The field to *filter* by: it must be declared. A value off its list is
    /// still searched (old tasks may carry it); the returned warning says so.
    pub fn check_filter(&self, name: &str, value: &str) -> Result<Option<String>> {
        let field = self.declared(name)?;
        let warning = field.values.as_ref().and_then(|values| {
            if values.iter().any(|v| v == value) {
                return None;
            }
            let hint = closest(value, values)
                .map(|v| format!(" — did you mean `{v}`?"))
                .unwrap_or_else(|| ".".to_string());
            Some(format!(
                "`{value}` isn't an allowed value for {name}{hint} Only tasks that still carry it will match."
            ))
        });
        Ok(warning)
    }

    /// The declared field called `name`, or a usage error that says what is
    /// declared — and where to declare more.
    pub fn declared(&self, name: &str) -> Result<&Field> {
        if let Some(field) = self.field(name) {
            return Ok(field);
        }
        if self.fields.is_empty() {
            return Err(Error::Usage(format!(
                "no fields are declared yet — declare `{name}` in {} (run `hippo-task fields` for an example)",
                self.path.display()
            )));
        }
        let names: Vec<String> = self.fields.iter().map(|f| f.name.clone()).collect();
        let hint = closest(name, &names)
            .map(|n| format!(" — did you mean `{n}`?"))
            .unwrap_or_else(|| ".".to_string());
        Err(Error::Usage(format!(
            "there's no field `{name}`{hint} Declared: {} (in {})",
            names.join(", "),
            self.path.display()
        )))
    }
}

/// One warning per setting this version doesn't know, suggesting the closest
/// one it does. `section` prefixes the name, e.g. `[format] `.
fn unknown<'a>(
    path: &Path,
    section: &str,
    settings: impl Iterator<Item = &'a String>,
    known: &[&str],
) -> Vec<String> {
    let known: Vec<String> = known.iter().map(|s| s.to_string()).collect();
    settings
        .map(|setting| {
            let hint = closest(setting, &known)
                .map(|k| format!(" — did you mean `{k}`?"))
                .unwrap_or_default();
            format!(
                "{}: ignored {section}`{setting}`, a setting hippo-task {} doesn't know{hint} (a newer hippo-task may use it)",
                path.display(),
                env!("CARGO_PKG_VERSION")
            )
        })
        .collect()
}

/// Validate `[format]` (ADR-012): required fields must be declared, and when
/// there's a template, every required section must be one of its headings —
/// so the template and the rule can't disagree.
fn format(raw: &RawFormat, fields: &[Field]) -> std::result::Result<Format, String> {
    let text = |name: &str, value: &Option<String>| match value {
        Some(v) if v.trim().is_empty() => Err(format!("`{name}` can't be empty")),
        _ => Ok(value.clone()),
    };
    let guide = text("guide", &raw.guide)?;
    let template = text("template", &raw.template)?;
    let required_fields = names("required_fields", &raw.required_fields)?;
    let declared: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
    for name in &required_fields {
        if !declared.contains(name) {
            let hint = closest(name, &declared)
                .map(|n| format!(" — did you mean `{n}`?"))
                .unwrap_or_else(|| format!(" — declare it first, under [fields.{name}]"));
            return Err(format!(
                "required_fields names `{name}`, which isn't a declared field{hint}"
            ));
        }
    }
    let required_sections = names("required_sections", &raw.required_sections)?;
    if let Some(template) = &template {
        let headings = crate::format::headings(template);
        for name in &required_sections {
            if !headings.contains(&crate::format::heading_key(name)) {
                return Err(format!(
                    "required section `{name}` isn't a heading in the template — add `## {name}` to it, or drop it from required_sections"
                ));
            }
        }
    }
    Ok(Format {
        guide,
        template,
        required_fields,
        required_sections,
    })
}

/// A list of names, trimmed: none empty, none twice.
fn names(setting: &str, list: &[String]) -> std::result::Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for name in list {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(format!("{setting} can't list an empty name"));
        }
        if out.contains(&name) {
            return Err(format!("{setting} lists `{name}` twice"));
        }
        out.push(name);
    }
    Ok(out)
}

/// `[media]` caps. A missing key keeps the default; zero is refused.
fn media_limits(path: &Path, raw: &RawMedia) -> Result<MediaLimits> {
    Ok(MediaLimits {
        max_image_bytes: mib(
            path,
            "max_image_mib",
            raw.max_image_mib,
            DEFAULT_MAX_IMAGE_MIB,
        )?,
        max_video_bytes: mib(
            path,
            "max_video_mib",
            raw.max_video_mib,
            DEFAULT_MAX_VIDEO_MIB,
        )?,
    })
}

fn mib(path: &Path, name: &str, value: Option<u64>, default_mib: u64) -> Result<u64> {
    let mib = value.unwrap_or(default_mib);
    if mib == 0 {
        return Err(Error::Usage(format!(
            "{}: [media] {name} must be a positive number of MiB",
            path.display()
        )));
    }
    mib.checked_mul(1024 * 1024)
        .ok_or_else(|| Error::Usage(format!("{}: [media] {name} is too large", path.display())))
}

/// Validate one field from the file.
fn field(name: String, raw: RawField) -> std::result::Result<Field, String> {
    let usable = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !usable {
        return Err(format!(
            "`{name}` can't be a field name — use lowercase letters, digits, `-` and `_`, starting with a letter (e.g. `project`, `customer-tier`)"
        ));
    }
    let values = match raw.values {
        None => None,
        Some(list) => {
            let mut values: Vec<String> = Vec::with_capacity(list.len());
            for v in list {
                let v = v.trim().to_string();
                if v.is_empty() {
                    return Err(format!("{name}: a value can't be empty"));
                }
                if values.contains(&v) {
                    return Err(format!("{name}: `{v}` is listed twice"));
                }
                values.push(v);
            }
            if values.is_empty() {
                return Err(format!(
                    "{name}: `values` lists nothing — list the allowed values, or remove `values` to allow any text"
                ));
            }
            Some(values)
        }
    };
    let display_name = match raw.display_name {
        Some(d) if d.trim().is_empty() => {
            return Err(format!("{name}: `display_name` can't be empty"))
        }
        Some(d) => d.trim().to_string(),
        None => default_display_name(&name),
    };
    Ok(Field {
        name,
        display_name,
        values,
    })
}

/// Every export column needs its own heading: duplicates break Notion's Import
/// and Merge with CSV (ADR-007). Compared ignoring case.
fn unique_headings(fields: &[Field]) -> std::result::Result<(), String> {
    let built_in = EXPORT_COLUMNS_BEFORE
        .iter()
        .chain(&EXPORT_COLUMNS_AFTER)
        .find_map(|column| {
            let f = fields
                .iter()
                .find(|f| f.display_name.to_lowercase() == column.to_lowercase())?;
            Some((f, *column))
        });
    if let Some((f, column)) = built_in {
        return Err(format!(
            "{}: its display name \"{}\" repeats the export's own {column} column — set a different `display_name` in [fields.{}]",
            f.name, f.display_name, f.name
        ));
    }
    for (i, a) in fields.iter().enumerate() {
        let same = |b: &&Field| b.display_name.to_lowercase() == a.display_name.to_lowercase();
        if let Some(b) = fields[i + 1..].iter().find(same) {
            return Err(format!(
                "{} and {} are both displayed as \"{}\" — give one a different `display_name`",
                a.name, b.name, a.display_name
            ));
        }
    }
    Ok(())
}

/// `customer-tier` → `Customer tier`: what a column heading says by default.
fn default_display_name(name: &str) -> String {
    let words = name.replace(['-', '_'], " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => words,
    }
}

// ----------------------------------------------------------- suggestions

/// The candidate someone most likely meant by `input`, if any is close:
/// the same word in another case, a word it starts (3+ characters typed), or
/// one within a few typos — about one per three characters.
pub fn closest<'a>(input: &str, candidates: &'a [String]) -> Option<&'a str> {
    let wanted = input.to_lowercase();
    if let Some(same) = candidates.iter().find(|c| c.to_lowercase() == wanted) {
        return Some(same);
    }
    if wanted.chars().count() >= 3 {
        if let Some(start) = candidates
            .iter()
            .find(|c| c.to_lowercase().starts_with(&wanted))
        {
            return Some(start);
        }
    }
    let allowed = (wanted.chars().count() / 3).max(1);
    candidates
        .iter()
        .map(|c| (edits(&wanted, &c.to_lowercase()), c))
        .filter(|(d, _)| *d <= allowed)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.as_str())
}

/// Levenshtein distance: the fewest single-character insertions, deletions or
/// substitutions that turn `a` into `b`. Rust note: one rolling row of the
/// classic table is enough, so this allocates one small `Vec`.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn edits_counts_single_character_changes() {
        assert_eq!(edits("project", "project"), 0);
        assert_eq!(edits("projct", "project"), 1);
        assert_eq!(edits("dashbaord", "dashboard"), 2);
        assert_eq!(edits("", "abc"), 3);
    }

    #[test]
    fn closest_finds_a_case_slip_a_prefix_or_a_typo_but_not_a_stranger() {
        let values = words(&["dashboard", "billing", "design"]);
        assert_eq!(closest("Dashboard", &values), Some("dashboard"));
        assert_eq!(closest("bil", &values), Some("billing"));
        assert_eq!(closest("dashbaord", &values), Some("dashboard"));
        assert_eq!(closest("ux", &values), None);
    }

    #[test]
    fn media_caps_default_to_eight_and_sixty_four_mib() {
        assert_eq!(DEFAULT_MAX_IMAGE_MIB, 8);
        assert_eq!(DEFAULT_MAX_VIDEO_MIB, 64);
        let limits = MediaLimits::default();
        assert_eq!(limits.max_image_bytes, 8 * 1024 * 1024);
        assert_eq!(limits.max_video_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn media_section_is_read_and_a_zero_cap_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "hippo-task-config-media-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(FILE);
        std::fs::write(&file, "[media]\nmax_image_mib = 2\n").unwrap();
        let config = Config::load(&dir).unwrap();
        assert!(config.warnings.is_empty(), "{:?}", config.warnings);
        assert_eq!(config.media.max_image_bytes, 2 * 1024 * 1024);
        assert_eq!(config.media.max_video_bytes, 64 * 1024 * 1024);

        std::fs::write(&file, "[media]\nmax_image_mib = 0\n").unwrap();
        let err = Config::load(&dir).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("max_image_mib"), "{err}");

        std::fs::write(&file, "[media]\nnope = 1\n").unwrap();
        let err = Config::load(&dir).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_display_name_defaults_to_the_name_in_words() {
        assert_eq!(default_display_name("project"), "Project");
        assert_eq!(default_display_name("customer-tier"), "Customer tier");
        assert_eq!(default_display_name("due_quarter"), "Due quarter");
    }
}
