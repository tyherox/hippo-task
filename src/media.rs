//! Description markdown (ADR-008): image links, files named by their bytes,
//! and the paragraph merge behind `desc --base`.
//!
//! The description itself stays the `body` string of `create` and `set-body`.
//! Nothing here is a new event. Files live in `<store>/media/<sha256>.<ext>`,
//! copied before the ledger is locked, and never deleted by a command — an
//! unreferenced file is harmless, and another writer may be about to link it.
//!
//! The CLI stores and prints the markdown source. It does not render HTML.

use crate::config::MediaLimits;
use crate::error::{Error, Result};
use crate::model::Task;
use crate::store::Store;
use pulldown_cmark::{Event, LinkType, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Where a relative image path is resolved from.
#[derive(Debug, Clone)]
pub enum Anchor {
    /// The current directory (`desc` text, `add --body`, `update --body`, stdin).
    Cwd,
    /// The directory of the `--file` the markdown was read from.
    Dir(PathBuf),
}

/// One store image link, derived from the markdown at read time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFile {
    pub caption: String,
    pub sha256: String,
    pub mime: &'static str,
    /// File length, or `None` when the store doesn't have the file.
    pub bytes: Option<u64>,
    /// Absolute path of `<store>/media/<sha256>.<ext>` on this machine.
    pub path: PathBuf,
}

/// One paragraph both sides rewrote differently. The whole description is
/// left unchanged; the command names these so the caller can redo the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub submitted: String,
    pub current: String,
}

const ACCEPTED: &str = "PNG, JPEG, GIF, WebP, MP4, QuickTime, or WebM";

/// ISO/IEC 14496-12 brands whose major brand means MP4 (stored as `.mp4`).
/// `iso2`–`iso6` are later base-media versions of `isom`; `mp41`/`mp42` are
/// the MP4 v1/v2 brands; `avc1` is AVC; `mp71` is MPEG-4 with MPEG-7 metadata.
/// QuickTime is the major brand `qt  ` (space-padded), stored as `.mov`.
const MP4_BRANDS: [&[u8; 4]; 10] = [
    b"isom", b"iso2", b"iso3", b"iso4", b"iso5", b"iso6", b"mp41", b"mp42", b"avc1", b"mp71",
];

/// Image brands that share the `ftyp` box with MP4. `msf1` is the HEIF image
/// sequence brand from the same spec as `mif1` (ISO/IEC 23008-12).
const REFUSED_BRANDS: [(&[u8; 4], &str); 6] = [
    (b"heic", "HEIC"),
    (b"heix", "HEIC"),
    (b"mif1", "HEIF"),
    (b"msf1", "HEIF"),
    (b"avif", "AVIF"),
    (b"avis", "AVIF"),
];

const STORE_EXTS: [&str; 7] = ["png", "jpg", "gif", "webp", "mp4", "mov", "webm"];

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Mp4,
    Mov,
    Webm,
}

impl Kind {
    fn ext(self) -> &'static str {
        match self {
            Kind::Png => "png",
            Kind::Jpeg => "jpg",
            Kind::Gif => "gif",
            Kind::Webp => "webp",
            Kind::Mp4 => "mp4",
            Kind::Mov => "mov",
            Kind::Webm => "webm",
        }
    }

    fn is_video(self) -> bool {
        matches!(self, Kind::Mp4 | Kind::Mov | Kind::Webm)
    }
}

fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

// --------------------------------------------------------------- normalize

/// LF line endings, no blank lines at either end, one blank line between
/// blocks outside a fenced code block. Blank lines inside a fence stay.
pub fn normalize(text: &str) -> String {
    blocks(text).join("\n\n")
}

/// Paragraphs: blocks split at blank lines outside fenced code.
pub fn paragraphs(text: &str) -> Vec<String> {
    blocks(text)
}

struct Fence {
    ch: u8,
    len: usize,
}

fn blocks(text: &str) -> Vec<String> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut fence: Option<Fence> = None;
    let mut gap = false;
    for line in text.split('\n') {
        if let Some(open) = &fence {
            current.push(line.to_string());
            if closes_fence(line, open) {
                fence = None;
            }
            continue;
        }
        if line.trim().is_empty() {
            if !current.is_empty() {
                gap = true;
            }
            continue;
        }
        if gap {
            out.push(std::mem::take(&mut current));
            gap = false;
        }
        if let Some(open) = open_fence(line) {
            fence = Some(open);
        }
        current.push(line.to_string());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.into_iter().map(|lines| lines.join("\n")).collect()
}

fn open_fence(line: &str) -> Option<Fence> {
    let rest = line.trim_start_matches(' ');
    let indent = line.len() - rest.len();
    if indent > 3 {
        return None;
    }
    let bytes = rest.as_bytes();
    let ch = *bytes.first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = bytes.iter().take_while(|b| **b == ch).count();
    if len < 3 {
        return None;
    }
    let info = &rest[len..];
    if ch == b'`' && info.contains('`') {
        return None;
    }
    Some(Fence { ch, len })
}

fn closes_fence(line: &str, fence: &Fence) -> bool {
    let rest = line.trim_start_matches(' ');
    let indent = line.len() - rest.len();
    if indent > 3 {
        return false;
    }
    let bytes = rest.as_bytes();
    let len = bytes.iter().take_while(|b| **b == fence.ch).count();
    len >= fence.len && rest[len..].trim().is_empty()
}

// ------------------------------------------------------------------- merge

/// Three-way merge of paragraphs. A change on only one side — an edit, an
/// add, or a removal — is kept. The same change on both sides is kept once.
/// Two different rewrites of the same paragraphs are a conflict: nothing is
/// written. Insertions at the same point are both kept, the current
/// description's new paragraphs first, then the submitted ones.
pub fn merge(
    base: &[String],
    ours: &[String],
    theirs: &[String],
) -> std::result::Result<Vec<String>, Vec<Conflict>> {
    let a = hunks(base, ours);
    let b = hunks(base, theirs);
    let mut out = Vec::new();
    let mut conflicts = Vec::new();
    let mut ia = 0;
    let mut ib = 0;
    let mut pos = 0;
    while ia < a.len() || ib < b.len() {
        if ia < a.len() && ib < b.len() {
            let ha = &a[ia];
            let hb = &b[ib];
            let same = ha.base_start == hb.base_start
                && ha.base_end == hb.base_end
                && ha.lines == hb.lines;
            if same {
                copy_base(&mut out, base, &mut pos, ha.base_start);
                out.extend(ha.lines.iter().cloned());
                pos = ha.base_end;
                ia += 1;
                ib += 1;
                continue;
            }
            if overlaps(ha, hb) {
                let start = ha.base_start.min(hb.base_start);
                let mut end = ha.base_end.max(hb.base_end);
                let mut submitted = ha.lines.clone();
                let mut current = hb.lines.clone();
                ia += 1;
                ib += 1;
                while ia < a.len() && a[ia].base_start < end {
                    end = end.max(a[ia].base_end);
                    submitted.extend(a[ia].lines.iter().cloned());
                    ia += 1;
                }
                while ib < b.len() && b[ib].base_start < end {
                    end = end.max(b[ib].base_end);
                    current.extend(b[ib].lines.iter().cloned());
                    ib += 1;
                }
                copy_base(&mut out, base, &mut pos, start);
                conflicts.push(Conflict {
                    submitted: show_lines(&submitted),
                    current: show_lines(&current),
                });
                pos = end;
                continue;
            }
        }
        let take_ours = match (a.get(ia), b.get(ib)) {
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (Some(ha), Some(hb)) => {
                ha.base_start < hb.base_start
                    || (ha.base_start == hb.base_start && ha.base_end < hb.base_end)
            }
            (None, None) => false,
        };
        let hunk = if take_ours { &a[ia] } else { &b[ib] };
        copy_base(&mut out, base, &mut pos, hunk.base_start);
        out.extend(hunk.lines.iter().cloned());
        pos = hunk.base_end;
        if take_ours {
            ia += 1;
        } else {
            ib += 1;
        }
    }
    if pos < base.len() {
        out.extend(base[pos..].iter().cloned());
    }
    if conflicts.is_empty() {
        Ok(out)
    } else {
        Err(conflicts)
    }
}

struct Hunk {
    base_start: usize,
    base_end: usize,
    lines: Vec<String>,
}

fn overlaps(a: &Hunk, b: &Hunk) -> bool {
    a.base_start < b.base_end && b.base_start < a.base_end
}

fn copy_base(out: &mut Vec<String>, base: &[String], pos: &mut usize, until: usize) {
    if *pos < until {
        let until = until.min(base.len());
        out.extend(base[*pos..until].iter().cloned());
        *pos = until;
    }
}

fn show_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        "(removed)".to_string()
    } else {
        lines.join("\n\n")
    }
}

fn hunks(base: &[String], edited: &[String]) -> Vec<Hunk> {
    let pairs = lcs(base, edited);
    let mut hunks = Vec::new();
    let mut bi = 0;
    let mut ei = 0;
    for (bm, em) in pairs {
        if bi < bm || ei < em {
            hunks.push(Hunk {
                base_start: bi,
                base_end: bm,
                lines: edited[ei..em].to_vec(),
            });
        }
        bi = bm + 1;
        ei = em + 1;
    }
    if bi < base.len() || ei < edited.len() {
        hunks.push(Hunk {
            base_start: bi,
            base_end: base.len(),
            lines: edited[ei..].to_vec(),
        });
    }
    hunks
}

/// Index pairs of the longest common subsequence, left to right.
fn lcs(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            if a[i] == b[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

// -------------------------------------------------------------------- links

#[derive(Debug)]
struct ImageLink {
    dest: String,
    caption: String,
    link_type: LinkType,
    /// Byte range of the inline destination, including `<...>` when used.
    dest_span: Option<std::ops::Range<usize>>,
}

fn images(markdown: &str) -> Vec<ImageLink> {
    let mut out = Vec::new();
    let mut stack: Vec<OpenImage> = Vec::new();
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        match event {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => {
                stack.push(OpenImage {
                    link_type,
                    dest: dest_url.into_string(),
                    caption: String::new(),
                    span: range.clone(),
                    inner_end: range.start,
                });
            }
            Event::End(TagEnd::Image) => {
                if let Some(open) = stack.pop() {
                    let dest_span = if open.link_type == LinkType::Inline {
                        destination_span(markdown, &open.span, open.inner_end)
                    } else {
                        None
                    };
                    out.push(ImageLink {
                        dest: open.dest,
                        caption: open.caption,
                        link_type: open.link_type,
                        dest_span,
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(open) = stack.last_mut() {
                    open.caption.push_str(&text);
                    open.inner_end = open.inner_end.max(range.end);
                }
            }
            _ => {
                if let Some(open) = stack.last_mut() {
                    open.inner_end = open.inner_end.max(range.end);
                }
            }
        }
    }
    out
}

struct OpenImage {
    link_type: LinkType,
    dest: String,
    caption: String,
    span: std::ops::Range<usize>,
    inner_end: usize,
}

fn destination_span(
    markdown: &str,
    span: &std::ops::Range<usize>,
    inner_end: usize,
) -> Option<std::ops::Range<usize>> {
    let from = inner_end.min(span.end).min(markdown.len());
    let end = span.end.min(markdown.len());
    let slice = markdown.get(from..end)?;
    let rel = slice.find("](")?;
    let dest_at = from + rel + 2;
    let (len, _) = scan_dest(markdown.get(dest_at..)?)?;
    Some(dest_at..dest_at + len)
}

/// Byte length of a CommonMark link destination, and the unescaped text.
/// `<dest>` includes the brackets in the length.
fn scan_dest(s: &str) -> Option<(usize, String)> {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'<') {
        let mut i = 1;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == b'>' {
                return Some((i + 1, out));
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] != b'\n' {
                i += 1;
            }
            if bytes[i] == b'\n' {
                return None;
            }
            let ch = s[i..].chars().next()?;
            out.push(ch);
            i += ch.len_utf8();
        }
        return None;
    }
    let mut i = 0;
    let mut out = String::new();
    let mut paren = 0i32;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && i + 1 < bytes.len() && bytes[i + 1] != b'\n' {
            i += 1;
            let ch = s[i..].chars().next()?;
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if c == b'(' {
            paren += 1;
            out.push('(');
            i += 1;
            continue;
        }
        if c == b')' {
            if paren == 0 {
                return Some((i, out));
            }
            paren -= 1;
            out.push(')');
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            return Some((i, out));
        }
        let ch = s[i..].chars().next()?;
        out.push(ch);
        i += ch.len_utf8();
    }
    Some((s.len(), out))
}

/// Text a search should see: prose, captions, and the URLs of links — a PR
/// or an issue is what you search for before filing a duplicate. Not image
/// destinations or store files: otherwise searching for `media` would match
/// every task with a file.
pub fn searchable(markdown: &str) -> String {
    let mut out = String::new();
    for event in Parser::new(markdown) {
        match event {
            // Alt text is a text event; an image's destination is not. Code is text too.
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) if store_name(&dest_url).is_none() => {
                out.push(' ');
                out.push_str(&dest_url);
                out.push(' ');
            }
            _ => {}
        }
    }
    out.to_lowercase()
}

fn apply_spans(text: &str, mut spans: Vec<(std::ops::Range<usize>, String)>) -> String {
    spans.sort_by_key(|span| std::cmp::Reverse(span.0.start));
    let mut out = text.to_string();
    for (range, replacement) in spans {
        if range.start <= range.end && range.end <= out.len() {
            out.replace_range(range, &replacement);
        }
    }
    out
}

// -------------------------------------------------------------------- files

/// `media/<64 hex>.<ext>` with a known extension, or `None`.
fn store_name(dest: &str) -> Option<String> {
    let dest = dest.strip_prefix("./").unwrap_or(dest);
    let file = dest.strip_prefix("media/")?;
    if file.contains('/') || file.contains('\\') {
        return None;
    }
    let (hash, ext) = file_name(file)?;
    Some(format!("media/{hash}.{ext}"))
}

fn file_name(name: &str) -> Option<(String, String)> {
    let (hash, ext) = name.rsplit_once('.')?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let ext = ext.to_ascii_lowercase();
    if !STORE_EXTS.contains(&ext.as_str()) {
        return None;
    }
    Some((hash.to_ascii_lowercase(), ext))
}

/// True when the markdown links a file in the store's `media/` folder.
pub fn has_store_media(markdown: &str) -> bool {
    images(markdown)
        .iter()
        .any(|image| store_name(&image.dest).is_some())
}

/// One entry per store image link, in reading order. URL images stay in the
/// markdown and are not entries. A missing file is still an entry (`bytes`
/// is `None`); `path` is where the file would be.
pub fn files_in(folder: &Path, markdown: &str) -> Vec<MediaFile> {
    images(markdown)
        .into_iter()
        .filter_map(|image| {
            let name = store_name(&image.dest)?;
            let file = name.strip_prefix("media/")?;
            let (hash, ext) = file_name(file)?;
            let path = folder.join("media").join(format!("{hash}.{ext}"));
            let path = std::path::absolute(&path).unwrap_or(path);
            let bytes = fs::metadata(&path)
                .ok()
                .filter(|meta| meta.is_file())
                .map(|meta| meta.len());
            Some(MediaFile {
                caption: image.caption,
                sha256: hash,
                mime: mime_of(&ext),
                bytes,
                path,
            })
        })
        .collect()
}

/// Warn when a linked file is missing. `hash` also re-reads the bytes and
/// warns when they don't match the name — `show` and `export` do that;
/// other commands only check that the file is there.
pub fn warn_about(store: &Store, tasks: &[Task], hash: bool) {
    for task in tasks {
        let Some(body) = task.body.as_deref() else {
            continue;
        };
        for file in files_in(store.folder(), body) {
            match file.bytes {
                None => store.warn(&format!(
                    "{}: missing media file {}",
                    task.handle(),
                    file.path.display()
                )),
                Some(_) if hash => match hash_file(&file.path) {
                    Ok(actual) if actual == file.sha256 => {}
                    Ok(actual) => store.warn(&format!(
                        "{}: {} does not match its name (the bytes hash to {actual})",
                        task.handle(),
                        file.path.display()
                    )),
                    Err(error) => store.warn(&format!(
                        "{}: couldn't read {}: {error}",
                        task.handle(),
                        file.path.display()
                    )),
                },
                Some(_) => {}
            }
        }
    }
}

/// Normalize `markdown`, copy local images into the store, and rewrite those
/// links to `media/<sha256>.<ext>`. Call this before taking the ledger lock.
/// A file already stored under its name is left alone. A copy that lands
/// before a later failure is left too: this function never deletes one.
pub fn ingest(
    store: &Store,
    markdown: &str,
    anchor: &Anchor,
    limits: &MediaLimits,
) -> Result<String> {
    let text = normalize(markdown);
    let found = images(&text);
    let anchor_dir = anchor.dir()?;
    let media_dir = store.folder().join("media");
    let mut replacements = Vec::new();
    let mut copies: Vec<PathBuf> = Vec::new();
    for image in &found {
        let dest = image.dest.as_str();
        if is_url(dest) {
            continue;
        }
        if let Some(name) = store_name(dest) {
            note_if_missing(store, &media_dir, &name);
            if let Some(span) = image.dest_span.clone() {
                if text.get(span.clone()) != Some(name.as_str()) {
                    replacements.push((span, name));
                }
            }
            continue;
        }
        // Only an inline link is rewritten. A reference (`![shot][s]` with
        // `[s]: shot.png`) would keep a path no other machine has.
        if image.link_type != LinkType::Inline {
            let caption = &image.caption;
            return Err(Error::Usage(format!(
                "![{caption}] refers to the local file {dest} — write the image inline, ![{caption}]({dest}), so the file can be copied into the store"
            )));
        }
        let local = resolve_local(dest, &anchor_dir);
        if let Some(name) = name_inside_media(&local, &media_dir) {
            note_if_missing(store, &media_dir, &name);
            if let Some(span) = image.dest_span.clone() {
                replacements.push((span, name));
            }
            continue;
        }
        let Some(span) = image.dest_span.clone() else {
            return Err(Error::Usage(format!(
                "couldn't read the image link to `{dest}`"
            )));
        };
        // Check every file before copying any, so a missing one leaves nothing new.
        inspect_local(&local, dest, limits)?;
        copies.push(local);
        replacements.push((span, String::new()));
    }
    let mut copy_at = 0;
    for replacement in &mut replacements {
        if replacement.1.is_empty() {
            let src = &copies[copy_at];
            copy_at += 1;
            replacement.1 = copy_in(src, &media_dir, limits)?;
        }
    }
    Ok(apply_spans(&text, replacements))
}

fn note_if_missing(store: &Store, media_dir: &Path, name: &str) {
    let file = name.strip_prefix("media/").unwrap_or(name);
    let path = media_dir.join(file);
    if !path.is_file() {
        store.warn(&format!(
            "{name} isn't in the store — the link was kept, but the file is missing"
        ));
    }
}

impl Anchor {
    fn dir(&self) -> Result<PathBuf> {
        match self {
            Anchor::Dir(dir) => Ok(dir.clone()),
            Anchor::Cwd => std::env::current_dir()
                .map_err(|e| Error::io("couldn't read the current folder", e)),
        }
    }
}

fn is_url(dest: &str) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    // A single letter is a Windows drive (`C:\…`), not a scheme.
    if scheme.chars().count() < 2 {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn resolve_local(dest: &str, anchor: &Path) -> PathBuf {
    let path = Path::new(dest);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        anchor.join(path)
    }
}

fn name_inside_media(path: &Path, media_dir: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let (hash, ext) = file_name(name)?;
    let parent = path.parent()?;
    if same_dir(parent, media_dir) {
        Some(format!("media/{hash}.{ext}"))
    } else {
        None
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => match (std::path::absolute(a), std::path::absolute(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        },
    }
}

fn inspect_local(path: &Path, name: &str, limits: &MediaLimits) -> Result<Kind> {
    let meta = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Usage(format!("no such file: {name}"))
        } else {
            Error::io(format!("couldn't read {name}"), error)
        }
    })?;
    if !meta.is_file() {
        return Err(Error::Usage(format!("{name} isn't a regular file")));
    }
    if meta.len() == 0 {
        return Err(Error::Usage(format!("{name} is empty")));
    }
    let header = read_header(path)?;
    let kind = sniff(&header).map_err(|why| Error::Usage(format!("{name}: {why}")))?;
    within_cap(name, meta.len(), kind, limits)?;
    Ok(kind)
}

/// Refuse a file over the cap for its type.
fn within_cap(name: &str, len: u64, kind: Kind, limits: &MediaLimits) -> Result<()> {
    let (cap, which) = if kind.is_video() {
        (limits.max_video_bytes, "video")
    } else {
        (limits.max_image_bytes, "image")
    };
    if len > cap {
        return Err(Error::Usage(format!(
            "{name} is {len} bytes, over the {cap}-byte limit for a {which} (raise it with [media] max_{which}_mib in config.toml)"
        )));
    }
    Ok(())
}

/// Copy a local file into the store. `inspect_local` checked it already, but
/// it may have changed since: the type, the cap and the name all come from
/// the one read that copies it, so the name is the hash of the bytes stored.
fn copy_in(src: &Path, media_dir: &Path, limits: &MediaLimits) -> Result<String> {
    let name = src.display().to_string();
    let limit = limits.max_image_bytes.max(limits.max_video_bytes);
    let copy = copy_to_temp(src, media_dir, Some(limit))?;
    if copy.len == 0 {
        return Err(Error::Usage(format!("{name} is empty")));
    }
    let kind = sniff(&copy.header).map_err(|why| Error::Usage(format!("{name}: {why}")))?;
    within_cap(&name, copy.len, kind, limits)?;
    let file_name = format!("{}.{}", copy.sha256, kind.ext());
    place(copy, &media_dir.join(&file_name))?;
    Ok(format!("media/{file_name}"))
}

enum Place {
    Already,
    Written,
}

/// A flushed copy under a temporary name, with the hash and length of exactly
/// the bytes it holds. Unless [`place`] renames it into place, dropping it
/// deletes it: a temporary name is never a stored file.
#[derive(Debug)]
struct TempCopy {
    path: PathBuf,
    sha256: String,
    len: u64,
    /// The first bytes, to read the type from.
    header: Vec<u8>,
}

impl Drop for TempCopy {
    fn drop(&mut self) {
        // Once renamed into place there's nothing here. Otherwise this is a
        // leftover that nothing links; failing to delete it costs disk space only.
        let _ = fs::remove_file(&self.path);
    }
}

/// Copy `src` into `dir` under a temporary name — hashing, counting, and
/// keeping the first bytes in the same pass — and flush it. Past `limit`
/// bytes it stops, and the partial copy is deleted.
fn copy_to_temp(src: &Path, dir: &Path, limit: Option<u64>) -> Result<TempCopy> {
    let mut input = File::open(src)
        .map_err(|error| Error::io(format!("couldn't read {}", src.display()), error))?;
    fs::create_dir_all(dir)
        .map_err(|error| Error::io(format!("couldn't create {}", dir.display()), error))?;
    // Declared before the output file, so on an early return the file is
    // closed before the copy deletes it (Windows can't delete an open file).
    let mut copy = TempCopy {
        path: dir.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
        )),
        sha256: String::new(),
        len: 0,
        header: Vec::with_capacity(HEADER_LEN),
    };
    let mut output = File::create(&copy.path)
        .map_err(|error| Error::io(format!("couldn't create {}", copy.path.display()), error))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = input
            .read(&mut buf)
            .map_err(|error| Error::io(format!("couldn't read {}", src.display()), error))?;
        if n == 0 {
            break;
        }
        copy.len += n as u64;
        if let Some(limit) = limit {
            if copy.len > limit {
                return Err(Error::Usage(format!(
                    "{} grew past {limit} bytes while it was being copied",
                    src.display()
                )));
            }
        }
        let chunk = &buf[..n];
        hasher.update(chunk);
        let room = HEADER_LEN.saturating_sub(copy.header.len());
        copy.header.extend_from_slice(&chunk[..room.min(n)]);
        output
            .write_all(chunk)
            .map_err(|error| Error::io(format!("couldn't write {}", copy.path.display()), error))?;
    }
    output
        .sync_all()
        .map_err(|error| Error::io(format!("couldn't flush {}", copy.path.display()), error))?;
    copy.sha256 = hex_encode(&hasher.finalize());
    Ok(copy)
}

/// Rename a temporary copy to `dest` and flush the folder. A file already
/// there with the same bytes stays, and the copy is dropped. Different bytes
/// are a usage error — the name is a hash, so a mismatch is a collision or a
/// damaged file, and it is not overwritten. Once renamed, the file stays even
/// if the flush fails.
fn place(copy: TempCopy, dest: &Path) -> Result<Place> {
    if dest.is_file() {
        return if holds(dest, copy.len, &copy.sha256)? {
            Ok(Place::Already)
        } else {
            Err(Error::Usage(format!(
                "{} already exists and its bytes differ — not overwritten",
                dest.display()
            )))
        };
    }
    fs::rename(&copy.path, dest).map_err(|error| {
        Error::io(
            format!(
                "couldn't rename {} to {}",
                copy.path.display(),
                dest.display()
            ),
            error,
        )
    })?;
    let parent = dest.parent().unwrap_or(Path::new("."));
    flush_dir(parent).map_err(|error| {
        Error::io(
            format!("couldn't flush the folder {}", parent.display()),
            error,
        )
    })?;
    Ok(Place::Written)
}

/// Does the file at `path` hold exactly the bytes with this length and hash?
fn holds(path: &Path, len: u64, sha256: &str) -> Result<bool> {
    let meta = fs::metadata(path)
        .map_err(|error| Error::io(format!("couldn't read {}", path.display()), error))?;
    if !meta.is_file() || meta.len() != len {
        return Ok(false);
    }
    let actual = hash_file(path)
        .map_err(|error| Error::io(format!("couldn't read {}", path.display()), error))?;
    Ok(actual == sha256)
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// How many leading bytes are read to tell a file's type.
const HEADER_LEN: usize = 4096;

fn read_header(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path)
        .map_err(|error| Error::io(format!("couldn't read {}", path.display()), error))?;
    let mut buf = vec![0u8; HEADER_LEN];
    let n = file
        .read(&mut buf)
        .map_err(|error| Error::io(format!("couldn't read {}", path.display()), error))?;
    buf.truncate(n);
    Ok(buf)
}

/// Copy the store files `markdowns` link to into `dest_dir`, skipping any
/// hash already in `handled` and adding each one this call deals with — so a
/// second call copies only what the first didn't. A file already there with
/// the same bytes is left alone and omitted from the result — the result is
/// only the files this call copied. Different bytes stop the copy.
///
/// A copied file is never deleted, even when the export then fails: copying
/// happens outside the ledger lock, so another export into the same folder
/// may already link it. Beside a CSV, a file imports nothing on its own.
pub fn copy_referenced(
    folder: &Path,
    markdowns: &[&str],
    dest_dir: &Path,
    handled: &mut BTreeSet<String>,
) -> Result<Vec<PathBuf>> {
    let mut copied = Vec::new();
    for markdown in markdowns {
        for file in files_in(folder, markdown) {
            if file.bytes.is_none() || !handled.insert(file.sha256.clone()) {
                continue;
            }
            let Some(name) = file.path.file_name() else {
                continue;
            };
            let dest = dest_dir.join(name);
            let copy = copy_to_temp(&file.path, dest_dir, None)?;
            if let Place::Written = place(copy, &dest)? {
                copied.push(dest);
            }
        }
    }
    Ok(copied)
}

#[cfg(not(windows))]
fn flush_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir).and_then(|file| file.sync_all())
}

#[cfg(windows)]
fn flush_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

// -------------------------------------------------------------------- sniff

fn sniff(header: &[u8]) -> std::result::Result<Kind, String> {
    if header.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Ok(Kind::Png);
    }
    if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(Kind::Jpeg);
    }
    if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        return Ok(Kind::Gif);
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return Ok(Kind::Webp);
    }
    if header.len() >= 12 && &header[4..8] == b"ftyp" {
        return sniff_ftyp(header);
    }
    if header.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return sniff_ebml(header);
    }
    Err(format!("not a {ACCEPTED} file"))
}

fn sniff_ftyp(data: &[u8]) -> std::result::Result<Kind, String> {
    if data.len() < 12 {
        return Err(format!("not a {ACCEPTED} file"));
    }
    let box_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let brand_end = if box_size >= 16 {
        box_size.min(data.len())
    } else {
        data.len()
    };
    let mut brands: Vec<&[u8]> = vec![&data[8..12]];
    let mut i = 16;
    while i + 4 <= brand_end {
        brands.push(&data[i..i + 4]);
        i += 4;
    }
    for brand in &brands {
        if let Some(name) = refused_name(brand) {
            return Err(format!("this is a {name} file, not a {ACCEPTED} file"));
        }
    }
    let major = &data[8..12];
    if major == b"qt  ".as_slice() {
        return Ok(Kind::Mov);
    }
    if MP4_BRANDS.iter().any(|brand| brand.as_slice() == major) {
        return Ok(Kind::Mp4);
    }
    let shown = String::from_utf8_lossy(major);
    Err(format!(
        "this is an ISO media file (brand \"{shown}\"), not a {ACCEPTED} file"
    ))
}

fn refused_name(brand: &[u8]) -> Option<&'static str> {
    REFUSED_BRANDS
        .iter()
        .find(|(bytes, _)| bytes.as_slice() == brand)
        .map(|(_, name)| *name)
}

fn sniff_ebml(data: &[u8]) -> std::result::Result<Kind, String> {
    let Some((id, next)) = read_id(data, 0) else {
        return Err("this is an EBML file, not WebM".into());
    };
    if id != 0x1A45_DFA3 {
        return Err("this is an EBML file, not WebM".into());
    }
    let Some((size, mut p)) = read_vint(data, next) else {
        return Err("this is an EBML file, not WebM".into());
    };
    let header_end = p.saturating_add(size as usize).min(data.len());
    while p < header_end {
        let Some((eid, n2)) = read_id(data, p) else {
            break;
        };
        let Some((sz, n3)) = read_vint(data, n2) else {
            break;
        };
        let content_end = n3.saturating_add(sz as usize);
        if eid == 0x4282 {
            let end = content_end.min(data.len());
            if n3 > end {
                break;
            }
            let doc = String::from_utf8_lossy(&data[n3..end]);
            if doc == "webm" {
                return Ok(Kind::Webm);
            }
            if doc == "matroska" {
                return Err("this is a Matroska file, not WebM".into());
            }
            return Err(format!(
                "this is an EBML file (DocType \"{doc}\"), not WebM"
            ));
        }
        if content_end <= p {
            break;
        }
        p = content_end;
    }
    Err("this is an EBML file, not WebM".into())
}

fn read_id(data: &[u8], i: usize) -> Option<(u64, usize)> {
    if i >= data.len() || data[i] == 0 {
        return None;
    }
    let width = data[i].leading_zeros() as usize + 1;
    if width > 4 || i + width > data.len() {
        return None;
    }
    let mut value = 0u64;
    for byte in &data[i..i + width] {
        value = (value << 8) | u64::from(*byte);
    }
    Some((value, i + width))
}

fn read_vint(data: &[u8], i: usize) -> Option<(u64, usize)> {
    if i >= data.len() || data[i] == 0 {
        return None;
    }
    let width = data[i].leading_zeros() as usize + 1;
    if width > 8 || i + width > data.len() {
        return None;
    }
    let mut value = u64::from(data[i] & (0xFF >> width));
    for byte in &data[i + 1..i + width] {
        value = (value << 8) | u64::from(*byte);
    }
    Some((value, i + width))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn png() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0]
    }

    fn ftyp(brand: &[u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 0, 20];
        bytes.extend_from_slice(b"ftyp");
        bytes.extend_from_slice(brand);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes
    }

    fn webm() -> Vec<u8> {
        vec![
            0x1A, 0x45, 0xDF, 0xA3, 0x87, 0x42, 0x82, 0x84, b'w', b'e', b'b', b'm',
        ]
    }

    fn matroska() -> Vec<u8> {
        let doc = b"matroska";
        let mut bytes = vec![0x1A, 0x45, 0xDF, 0xA3, 0x8B, 0x42, 0x82, 0x88];
        bytes.extend_from_slice(doc);
        bytes
    }

    #[test]
    fn sniff_accepts_images_and_video_and_names_the_refusals() {
        assert_eq!(sniff(&png()).unwrap(), Kind::Png);
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xD9]).unwrap(), Kind::Jpeg);
        assert_eq!(sniff(b"GIF89a").unwrap(), Kind::Gif);
        assert_eq!(sniff(b"GIF87a....").unwrap(), Kind::Gif);
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&4u32.to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        assert_eq!(sniff(&webp).unwrap(), Kind::Webp);
        assert_eq!(sniff(&ftyp(b"isom")).unwrap(), Kind::Mp4);
        assert_eq!(sniff(&ftyp(b"mp42")).unwrap(), Kind::Mp4);
        assert_eq!(sniff(&ftyp(b"qt  ")).unwrap(), Kind::Mov);
        assert_eq!(sniff(&webm()).unwrap(), Kind::Webm);
        assert_eq!(Kind::Jpeg.ext(), "jpg");

        let heic = sniff(&ftyp(b"heic")).unwrap_err();
        assert!(heic.contains("HEIC"), "{heic}");
        let avif = sniff(&ftyp(b"avif")).unwrap_err();
        assert!(avif.contains("AVIF"), "{avif}");
        let heif = sniff(&ftyp(b"mif1")).unwrap_err();
        assert!(heif.contains("HEIF"), "{heif}");
        let mkv = sniff(&matroska()).unwrap_err();
        assert!(mkv.contains("Matroska"), "{mkv}");
        let random = sniff(b"not a picture").unwrap_err();
        assert!(random.contains("not a"), "{random}");
        let odd = sniff(&ftyp(b"M4A ")).unwrap_err();
        assert!(odd.contains("M4A"), "{odd}");
    }

    #[test]
    fn normalization_collapses_blank_lines_outside_fences_only() {
        assert_eq!(normalize("a\r\n\r\n\r\nb\n"), "a\n\nb");
        assert_eq!(normalize("\n\n  hello  \n\n"), "  hello  ");
        assert_eq!(normalize(""), "");
        let fenced = "para\n\n\n```\ncode\n\n\nstill\n```\n\n\nnext\n";
        assert_eq!(
            normalize(fenced),
            "para\n\n```\ncode\n\n\nstill\n```\n\nnext"
        );
        assert_eq!(
            paragraphs("alpha\n\nbeta"),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn merge_keeps_one_sided_edits_and_conflicts_on_a_disagreement() {
        let base = vec!["alpha".into(), "beta".into()];
        let edited = merge(&base, &["alpha".into(), "BETA".into()], &base).unwrap();
        assert_eq!(edited, vec!["alpha".to_string(), "BETA".to_string()]);

        let both = merge(
            &["alpha".into()],
            &["alpha".into(), "from ours".into()],
            &["alpha".into(), "gamma".into()],
        )
        .unwrap();
        assert_eq!(
            both,
            vec![
                "alpha".to_string(),
                "gamma".to_string(),
                "from ours".to_string()
            ]
        );

        let same = merge(
            &base,
            &["alpha".into(), "BETA".into()],
            &["alpha".into(), "BETA".into()],
        )
        .unwrap();
        assert_eq!(same, vec!["alpha".to_string(), "BETA".to_string()]);

        let removed = merge(
            &["alpha".into(), "beta".into(), "gamma".into()],
            &["alpha".into(), "gamma".into()],
            &[
                "alpha".into(),
                "beta".into(),
                "gamma".into(),
                "delta".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            removed,
            vec![
                "alpha".to_string(),
                "gamma".to_string(),
                "delta".to_string()
            ]
        );

        let conflict = merge(
            &base,
            &["alpha".into(), "OURS".into()],
            &["alpha".into(), "THEIRS".into()],
        )
        .unwrap_err();
        assert_eq!(conflict.len(), 1);
        assert!(conflict[0].submitted.contains("OURS"), "{conflict:?}");
        assert!(conflict[0].current.contains("THEIRS"), "{conflict:?}");
    }

    #[test]
    fn code_is_not_an_image_link_and_urls_stay_text() {
        let fenced = "see `![x](file.png)`\n\n```\n![x](file.png)\n```\n\n    ![x](file.png)\n";
        assert!(images(fenced).is_empty(), "{:?}", images(fenced));
        let url = "![remote](https://example.com/a.png)";
        let found = images(url);
        assert_eq!(found.len(), 1);
        assert!(is_url(&found[0].dest));
        assert_eq!(found[0].caption, "remote");
        let angled = "![shot](<my file.png>)";
        let found = images(angled);
        assert_eq!(found[0].dest, "my file.png");
        let span = found[0].dest_span.clone().unwrap();
        assert_eq!(&angled[span], "<my file.png>");
    }

    #[test]
    fn search_text_keeps_captions_and_drops_destinations() {
        let body = "the login fails\n\n![the login box](media/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png)";
        let text = searchable(body);
        assert!(text.contains("login fails"), "{text}");
        assert!(text.contains("login box"), "{text}");
        assert!(!text.contains("media/"), "{text}");
        assert!(!text.contains("aaaa"), "{text}");
    }

    #[test]
    fn search_text_keeps_a_link_url_but_not_a_store_file() {
        let store = format!("media/{}.png", "b".repeat(64));
        let body = format!(
            "fixed in [the PR](https://github.com/acme/app/pull/4821)\n\n[raw]({store}) and ![x](https://example.com/a.png)"
        );
        let text = searchable(&body);
        assert!(text.contains("the pr"), "{text}");
        assert!(text.contains("github.com/acme/app/pull/4821"), "{text}");
        assert!(text.contains("raw"), "{text}");
        assert!(!text.contains("media/"), "{text}");
        assert!(
            !text.contains("example.com"),
            "an image's target is not searched: {text}"
        );
    }

    fn temp_names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".tmp-"))
            .collect()
    }

    #[test]
    fn a_copy_names_the_bytes_it_copied_and_stops_at_its_limit() {
        let (dir, _store) = temp_store("copy-limit");
        let src = dir.0.join("shot.png");
        fs::write(&src, png()).unwrap();
        let out = dir.0.join("out");

        let copy = copy_to_temp(&src, &out, Some(10)).unwrap();
        assert_eq!(copy.len, 10);
        assert_eq!(copy.header, png());
        assert_eq!(copy.sha256, hash_file(&src).unwrap());
        drop(copy);
        assert!(
            temp_names(&out).is_empty(),
            "an unplaced copy deletes itself"
        );

        let over = copy_to_temp(&src, &out, Some(9));
        assert!(matches!(over, Err(Error::Usage(_))), "{over:?}");
        assert!(temp_names(&out).is_empty(), "a refused copy leaves nothing");
    }

    #[test]
    fn placing_bytes_already_there_keeps_the_file_and_drops_the_copy() {
        let (dir, _store) = temp_store("place");
        let src = dir.0.join("shot.png");
        fs::write(&src, png()).unwrap();
        let out = dir.0.join("out");
        let dest = out.join("kept.png");

        let first = copy_to_temp(&src, &out, None).unwrap();
        assert!(matches!(place(first, &dest), Ok(Place::Written)));
        let second = copy_to_temp(&src, &out, None).unwrap();
        assert!(matches!(place(second, &dest), Ok(Place::Already)));
        assert!(temp_names(&out).is_empty(), "{:?}", temp_names(&out));

        fs::write(&dest, b"other bytes").unwrap();
        let third = copy_to_temp(&src, &out, None).unwrap();
        assert!(matches!(place(third, &dest), Err(Error::Usage(_))));
        assert_eq!(fs::read(&dest).unwrap(), b"other bytes", "not overwritten");
        assert!(temp_names(&out).is_empty(), "{:?}", temp_names(&out));
    }

    fn temp_store(tag: &str) -> (TempfileDir, Store) {
        let dir = std::env::temp_dir().join(format!(
            "hippo-media-{tag}-{}-{}",
            std::process::id(),
            TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        (TempfileDir(dir.clone()), Store::new(&dir))
    }

    struct TempfileDir(PathBuf);
    impl Drop for TempfileDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_local_image_is_copied_once_and_a_missing_one_is_named() {
        let (dir, store) = temp_store("copy");
        let shot = dir.0.join("shot.png");
        fs::write(&shot, png()).unwrap();
        let limits = MediaLimits {
            max_image_bytes: 1024,
            max_video_bytes: 1024,
        };
        let body = ingest(
            &store,
            "see\n\n![the 401](shot.png)\n",
            &Anchor::Dir(dir.0.clone()),
            &limits,
        )
        .unwrap();
        assert!(body.contains("![the 401](media/"), "{body}");
        assert!(body.ends_with(".png") || body.contains(".png)"), "{body}");
        let again = ingest(
            &store,
            &body.replace("the 401", "same bytes"),
            &Anchor::Cwd,
            &limits,
        )
        .unwrap();
        let media = store.folder().join("media");
        let files: Vec<_> = fs::read_dir(&media)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| !n.to_string_lossy().starts_with('.'))
            .collect();
        assert_eq!(
            files.len(),
            1,
            "shared bytes are one file: {files:?} {again}"
        );

        let missing = ingest(
            &store,
            "![x](nope.png)",
            &Anchor::Dir(dir.0.clone()),
            &limits,
        );
        let err = missing.unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("nope.png"), "{err}");

        fs::write(dir.0.join("empty.png"), b"").unwrap();
        let err = ingest(
            &store,
            "![x](empty.png)",
            &Anchor::Dir(dir.0.clone()),
            &limits,
        )
        .unwrap_err();
        assert!(err.to_string().contains("empty"), "{err}");

        fs::create_dir(dir.0.join("adir")).unwrap();
        let err = ingest(&store, "![x](adir)", &Anchor::Dir(dir.0.clone()), &limits).unwrap_err();
        assert!(err.to_string().contains("regular"), "{err}");

        let big = dir.0.join("big.png");
        fs::write(&big, png()).unwrap();
        File::options()
            .write(true)
            .open(&big)
            .unwrap()
            .set_len(50)
            .unwrap();
        let tiny = MediaLimits {
            max_image_bytes: 16,
            max_video_bytes: 16,
        };
        let err = ingest(&store, "![x](big.png)", &Anchor::Dir(dir.0.clone()), &tiny).unwrap_err();
        assert!(err.to_string().contains("limit"), "{err}");

        let url = ingest(
            &store,
            "![remote](https://example.com/a.png)",
            &Anchor::Dir(dir.0.clone()),
            &limits,
        )
        .unwrap();
        assert_eq!(url, "![remote](https://example.com/a.png)");

        let coded = ingest(
            &store,
            "run `![x](missing.png)`\n",
            &Anchor::Dir(dir.0.clone()),
            &limits,
        )
        .unwrap();
        assert!(coded.contains("missing.png"), "{coded}");
    }
}
