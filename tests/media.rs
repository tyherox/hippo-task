//! Descriptions as markdown (ADR-008), through the CLI: files named by their
//! bytes, a paragraph merge, and the files an export copies beside its CSV.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{cmd, ok, tasks, TempDir, CLAUDE, CODEX, HUMAN};
use hippo_task::store::Store;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
const MIB: usize = 1024 * 1024;

fn init(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    ok(dir.path(), HUMAN, &["init", "--here"]);
    dir
}

fn shown(dir: &Path, id: &str) -> Value {
    ok(dir, HUMAN, &["show", id, "--json"]).json()
}

fn body_of(dir: &Path, id: &str) -> String {
    shown(dir, id)["body"].as_str().unwrap_or("").to_string()
}

fn media_dir(dir: &Path) -> std::path::PathBuf {
    dir.join(".hippotask").join("media")
}

fn stored_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let media = media_dir(dir);
    if !media.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<_> = fs::read_dir(&media)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
        })
        .collect();
    files.sort();
    files
}

fn ftyp(brand: &[u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 20];
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(brand);
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes
}

fn write_config(dir: &Path, text: &str) {
    fs::write(dir.join(".hippotask").join("config.toml"), text).unwrap();
}

/// An MP4 header padded to `len` bytes of `fill` — big enough that hashing
/// and copying it takes a while in a debug build.
fn video(path: &Path, len: usize, fill: u8) {
    let mut bytes = ftyp(b"isom");
    bytes.resize(len, fill);
    fs::write(path, &bytes).unwrap();
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Names in the store's `media/` folder that start with a dot: temporary copies.
fn temporary_files(dir: &Path) -> Vec<String> {
    fs::read_dir(media_dir(dir))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect()
}

#[test]
fn a_local_image_is_copied_once_and_a_url_is_left_alone() {
    let dir = init("copy");
    let d = dir.path();
    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let md = format!("the refresh call fails\n\n![the 401]({})", shot.display());
    ok(d, HUMAN, &["add", "bug", "--body", &md]);
    ok(d, HUMAN, &["add", "same bytes", "--body", &md]);

    let files = stored_files(d);
    assert_eq!(
        files.len(),
        1,
        "two links to the same bytes share one file: {files:?}"
    );
    assert_eq!(fs::read(&files[0]).unwrap(), PNG);
    let name = files[0].file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with(".png"), "{name}");
    assert_eq!(name.len(), 64 + 4, "sha256 plus .png: {name}");

    let body = body_of(d, "1");
    assert!(
        body.contains(&format!("![the 401](media/{name})")),
        "{body}"
    );
    assert!(!body.contains(&shot.display().to_string()), "{body}");

    let task = shown(d, "1");
    let media = task["media"].as_array().unwrap();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0]["caption"], "the 401");
    assert_eq!(media[0]["sha256"], name.trim_end_matches(".png"));
    assert_eq!(media[0]["mime"], "image/png");
    assert_eq!(media[0]["bytes"], PNG.len() as u64);
    assert_eq!(media[0]["path"], files[0].display().to_string());

    let jpeg = d.join("photo.JPEG");
    fs::write(&jpeg, [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
    let md = format!("![camera]({})", jpeg.display());
    ok(d, HUMAN, &["update", "2", "--body", &md]);
    let body = body_of(d, "2");
    assert!(body.contains(".jpg)"), "JPEG is stored as .jpg: {body}");
    assert!(!body.contains(".JPEG"), "{body}");

    ok(
        d,
        HUMAN,
        &["desc", "2", "![remote](https://example.com/a.png)"],
    );
    let task = shown(d, "2");
    assert_eq!(task["body"], "![remote](https://example.com/a.png)");
    assert_eq!(task["media"].as_array().unwrap().len(), 0);
}

#[test]
fn code_is_not_ingested_and_a_missing_file_is_named() {
    let dir = init("code");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t"]);
    let coded = "see `![x](nope.png)`\n\n```\n![x](nope.png)\n```\n\n    ![x](nope.png)";
    let task = ok(d, HUMAN, &["desc", "1", coded, "--json"]).json();
    let body = task["body"].as_str().unwrap();
    assert!(body.contains("nope.png"), "{body}");
    assert!(stored_files(d).is_empty());

    let missing = d.join("gone.png");
    let md = format!("![x]({})", missing.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    let err = run.json_error();
    assert_eq!(err["error"], "usage");
    assert!(
        err["message"].as_str().unwrap().contains("gone.png"),
        "{err}"
    );
    assert!(
        body_of(d, "1").contains("nope.png"),
        "a refusal writes nothing"
    );

    fs::write(d.join("empty.png"), b"").unwrap();
    let md = format!("![x]({})", d.join("empty.png").display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2);
    assert!(run.json_error()["message"]
        .as_str()
        .unwrap()
        .contains("empty"));

    fs::create_dir(d.join("adir")).unwrap();
    let md = format!("![x]({})", d.join("adir").display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2);
    assert!(
        run.json_error()["message"]
            .as_str()
            .unwrap()
            .contains("regular"),
        "{run:#?}"
    );

    let heic = d.join("photo.heic");
    fs::write(&heic, ftyp(b"heic")).unwrap();
    let md = format!("![x]({})", heic.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2);
    assert!(
        run.json_error()["message"]
            .as_str()
            .unwrap()
            .contains("HEIC"),
        "{run:#?}"
    );

    let junk = d.join("notes.bin");
    fs::write(&junk, b"not a picture").unwrap();
    let md = format!("![x]({})", junk.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2);
    assert!(
        run.json_error()["message"]
            .as_str()
            .unwrap()
            .contains("not a"),
        "{run:#?}"
    );
}

#[test]
fn a_file_argument_resolves_images_beside_itself_and_stdin_uses_the_current_folder() {
    let dir = init("file");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t"]);
    let notes = d.join("notes");
    fs::create_dir(&notes).unwrap();
    fs::write(notes.join("my shot.png"), PNG).unwrap();
    let note = notes.join("note.md");
    fs::write(&note, "before\n\n![the login](<my shot.png>)\n\nafter\n").unwrap();
    let task = ok(
        d,
        HUMAN,
        &["desc", "1", "--file", note.to_str().unwrap(), "--json"],
    )
    .json();
    let body = task["body"].as_str().unwrap();
    assert!(body.contains("![the login](media/"), "{body}");
    assert!(!body.contains("my shot.png"), "{body}");
    assert_eq!(task["media"][0]["caption"], "the login");

    fs::write(d.join("shot.png"), PNG).unwrap();
    let mut child = cmd(d, HUMAN, &["desc", "1", "--file", "-", "--json"])
        .current_dir(d)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"from stdin\n\n![shot](shot.png)\n")
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let task: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        task["body"].as_str().unwrap().starts_with("from stdin"),
        "{task}"
    );
    assert!(task["body"].as_str().unwrap().contains("media/"), "{task}");
    assert_eq!(
        stored_files(d).len(),
        1,
        "the same bytes are still one file"
    );
}

#[test]
fn caps_come_from_config_and_a_video_uses_the_video_cap() {
    let dir = init("caps");
    let d = dir.path();
    write_config(d, "[media]\nmax_image_mib = 1\n");
    ok(d, HUMAN, &["add", "t", "--body", "plain text"]);
    let run = ok(d, HUMAN, &["add", "also", "--json"]);
    assert!(
        !run.stderr.contains("ignored `media`"),
        "a known [media] section is not a warning: {}",
        run.stderr
    );

    let big = d.join("big.png");
    fs::write(&big, PNG).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .unwrap()
        .set_len(2 * 1024 * 1024)
        .unwrap();
    let md = format!("![x]({})", big.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    let message = run.json_error()["message"].as_str().unwrap().to_string();
    assert!(message.contains("big.png"), "{message}");
    assert!(message.contains("limit"), "{message}");

    let video = d.join("clip.mov");
    fs::write(&video, ftyp(b"qt  ")).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&video)
        .unwrap()
        .set_len(2 * 1024 * 1024)
        .unwrap();
    let md = format!("![recording]({})", video.display());
    let task = ok(d, HUMAN, &["desc", "1", &md, "--json"]).json();
    assert!(
        task["body"].as_str().unwrap().contains(".mov)"),
        "2 MiB is over the image cap and under the video cap: {task}"
    );
    assert_eq!(task["media"][0]["mime"], "video/quicktime");

    write_config(d, "[media]\nnope = 1\n");
    let run = tasks(d, HUMAN, &["desc", "1", "text", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.json_error()["message"]
        .as_str()
        .unwrap()
        .contains("nope"));
}

#[test]
fn newlines_and_blank_lines_are_normalized() {
    let dir = init("norm");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t"]);
    let task = ok(
        d,
        HUMAN,
        &["desc", "1", "hello\r\n\r\n\r\nworld\n\n", "--json"],
    )
    .json();
    assert_eq!(task["body"], "hello\n\nworld");
}

#[test]
fn base_merges_by_paragraph_and_a_disagreement_writes_nothing() {
    let dir = init("merge");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t", "--body", "alpha\n\nbeta"]);
    assert_eq!(shown(d, "1")["seq"], 1);

    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\nBETA"]);
    let merged = ok(
        d,
        HUMAN,
        &["desc", "1", "--base", "1", "ALPHA\n\nbeta", "--json"],
    )
    .json();
    assert_eq!(merged["body"], "ALPHA\n\nBETA");

    ok(d, HUMAN, &["update", "1", "--body", "alpha"]);
    let seq = shown(d, "1")["seq"].as_u64().unwrap();
    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\ngamma"]);
    let both = ok(
        d,
        HUMAN,
        &[
            "desc",
            "1",
            "--base",
            &seq.to_string(),
            "alpha\n\nfrom ours",
            "--json",
        ],
    )
    .json();
    assert_eq!(both["body"], "alpha\n\ngamma\n\nfrom ours");

    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\nbeta"]);
    let seq = shown(d, "1")["seq"].as_u64().unwrap();
    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\nBETA"]);
    let same = ok(
        d,
        HUMAN,
        &[
            "desc",
            "1",
            "--base",
            &seq.to_string(),
            "alpha\n\nBETA",
            "--json",
        ],
    )
    .json();
    assert_eq!(same["body"], "alpha\n\nBETA");

    ok(
        d,
        HUMAN,
        &["update", "1", "--body", "alpha\n\nbeta\n\ngamma"],
    );
    let seq = shown(d, "1")["seq"].as_u64().unwrap();
    ok(
        d,
        HUMAN,
        &["update", "1", "--body", "alpha\n\nbeta\n\ngamma\n\ndelta"],
    );
    let removed = ok(
        d,
        HUMAN,
        &[
            "desc",
            "1",
            "--base",
            &seq.to_string(),
            "alpha\n\ngamma",
            "--json",
        ],
    )
    .json();
    assert_eq!(removed["body"], "alpha\n\ngamma\n\ndelta");

    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\nbeta"]);
    let seq = shown(d, "1")["seq"].as_u64().unwrap();
    ok(d, HUMAN, &["update", "1", "--body", "alpha\n\nTHEIRS"]);
    let before = dir.ledger_text();
    let run = tasks(
        d,
        HUMAN,
        &[
            "desc",
            "1",
            "--base",
            &seq.to_string(),
            "alpha\n\nOURS",
            "--json",
        ],
    );
    assert_eq!(run.code, 5, "{run:#?}");
    let err = run.json_error();
    assert_eq!(err["error"], "stale");
    assert_eq!(err["exit_code"], 5);
    let message = err["message"].as_str().unwrap();
    assert!(message.contains("OURS"), "{message}");
    assert!(message.contains("THEIRS"), "{message}");
    assert_eq!(dir.ledger_text(), before, "a conflict appends nothing");
    assert_eq!(body_of(d, "1"), "alpha\n\nTHEIRS");

    let run = tasks(d, HUMAN, &["desc", "1", "--base", "0", "nope", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert_eq!(run.json_error()["error"], "usage");
    let run = tasks(d, HUMAN, &["desc", "1", "--base", "99", "nope", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert_eq!(body_of(d, "1"), "alpha\n\nTHEIRS");

    ok(d, HUMAN, &["desc", "1", "replaced outright"]);
    assert_eq!(body_of(d, "1"), "replaced outright");
}

#[test]
fn anyone_may_edit_the_description() {
    let dir = init("collab");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t", "--body", "original"]);
    ok(d, CLAUDE, &["start", "1"]);
    let task = ok(d, CODEX, &["desc", "1", "edited by someone else", "--json"]).json();
    assert_eq!(task["body"], "edited by someone else");
    assert_eq!(task["state"], "doing");
    assert_eq!(task["lease"]["holder"], "agent:claude");
}

#[test]
fn search_matches_captions_and_prose_but_not_the_file_path() {
    let dir = init("search");
    let d = dir.path();
    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let md = format!(
        "the refresh call fails\n\n![the login box]({})",
        shot.display()
    );
    ok(d, HUMAN, &["add", "bug", "--body", &md]);
    ok(
        d,
        HUMAN,
        &[
            "add",
            "remote",
            "--body",
            "![remote](https://example.com/a.png)",
        ],
    );

    let media = ok(d, HUMAN, &["list", "--search", "media", "--json"]).json();
    assert_eq!(media.as_array().unwrap().len(), 0, "{media}");
    let png = ok(d, HUMAN, &["list", "--search", "png", "--json"]).json();
    assert_eq!(png.as_array().unwrap().len(), 0, "{png}");
    let caption = ok(d, HUMAN, &["list", "--search", "login box", "--json"]).json();
    assert_eq!(caption.as_array().unwrap().len(), 1);
    let prose = ok(d, HUMAN, &["list", "--search", "refresh", "--json"]).json();
    assert_eq!(prose.as_array().unwrap().len(), 1);
    let url = ok(d, HUMAN, &["list", "--search", "example.com", "--json"]).json();
    assert_eq!(url.as_array().unwrap().len(), 0, "{url}");
}

#[test]
fn a_missing_file_has_null_bytes_and_only_show_hashes() {
    let dir = init("warn");
    let d = dir.path();
    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let md = format!("![the 401]({})", shot.display());
    ok(d, HUMAN, &["add", "bug", "--body", &md]);
    let stored = stored_files(d);
    assert_eq!(stored.len(), 1);

    fs::write(&stored[0], b"not the original bytes").unwrap();
    let listed = ok(d, HUMAN, &["list", "--json"]);
    assert!(
        !listed.stderr.contains("does not match"),
        "list does not hash: {}",
        listed.stderr
    );
    assert!(!listed.stderr.contains("missing"), "{}", listed.stderr);
    let shown_run = ok(d, HUMAN, &["show", "1", "--json"]);
    assert!(
        shown_run.stderr.contains("does not match"),
        "{}",
        shown_run.stderr
    );

    fs::remove_file(&stored[0]).unwrap();
    let listed = ok(d, HUMAN, &["list", "--json"]);
    assert!(listed.stderr.contains("missing"), "{}", listed.stderr);
    assert!(
        !listed.stderr.contains("does not match"),
        "{}",
        listed.stderr
    );
    let task = shown(d, "1");
    assert!(task["media"][0]["bytes"].is_null(), "{task}");
    assert!(task["media"][0]["path"]
        .as_str()
        .unwrap()
        .ends_with(stored[0].file_name().unwrap().to_str().unwrap()));
    let text = ok(d, HUMAN, &["show", "1"]);
    assert!(text.stdout.contains("file:"), "{}", text.stdout);
    assert!(text.stdout.contains("the 401"), "{}", text.stdout);
}

#[test]
fn export_copies_media_beside_the_csv_and_refuses_different_bytes() {
    let dir = init("export-media");
    let d = dir.path();
    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let md = format!("see\n\n![the 401]({})", shot.display());
    ok(d, HUMAN, &["add", "bug", "--body", &md]);

    let csv = d.join("tasks.csv");
    let report = ok(
        d,
        HUMAN,
        &["export", "notion", "--out", csv.to_str().unwrap()],
    );
    assert!(
        report.stdout.contains("added by hand"),
        "the report says the files stay on disk: {}",
        report.stdout
    );
    let beside = d.join("media");
    let copied: Vec<_> = fs::read_dir(&beside)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(copied.len(), 1, "{copied:?}");
    assert_eq!(fs::read(&copied[0]).unwrap(), PNG);
    let csv_text = fs::read_to_string(&csv).unwrap();
    assert!(csv_text.contains("media/"), "{csv_text}");
    assert!(csv_text.contains("the 401"), "{csv_text}");

    let again = d.join("again.csv");
    let run = ok(
        d,
        HUMAN,
        &[
            "export",
            "notion",
            "--out",
            again.to_str().unwrap(),
            "--again",
            "--json",
        ],
    );
    let report = run.json();
    assert_eq!(
        report["media_files"].as_array().unwrap().len(),
        0,
        "a file already there with the same bytes was not copied: {report}"
    );

    let fresh = d.join("out");
    fs::create_dir(&fresh).unwrap();
    let fresh_csv = fresh.join("tasks.csv");
    let run = ok(
        d,
        HUMAN,
        &[
            "export",
            "notion",
            "--out",
            fresh_csv.to_str().unwrap(),
            "--again",
            "--json",
        ],
    );
    let files = run.json()["media_files"].as_array().unwrap().clone();
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(
        files[0].as_str().unwrap(),
        fresh
            .join("media")
            .join(copied[0].file_name().unwrap())
            .display()
            .to_string()
    );

    fs::write(&copied[0], b"different bytes").unwrap();
    let third = d.join("third.csv");
    let run = tasks(
        d,
        HUMAN,
        &[
            "export",
            "notion",
            "--out",
            third.to_str().unwrap(),
            "--again",
            "--json",
        ],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(
        run.json_error()["message"]
            .as_str()
            .unwrap()
            .contains("differ"),
        "{run:#?}"
    );
    assert!(!third.exists(), "a refused export writes no CSV");
    assert_eq!(fs::read(&copied[0]).unwrap(), b"different bytes");

    // Changing the file on disk is not a row change; changing the paragraph is.
    fs::write(&copied[0], PNG).unwrap();
    let preview = ok(d, HUMAN, &["export", "notion"]);
    assert!(
        !preview.stderr.contains("changed since"),
        "{}",
        preview.stderr
    );
    let link = copied[0].file_name().unwrap().to_str().unwrap();
    let md = format!("a new paragraph\n\n![the 401](media/{link})");
    ok(d, HUMAN, &["update", "1", "--body", &md]);
    let preview = ok(d, HUMAN, &["export", "notion"]);
    assert!(
        preview.stderr.contains("changed since"),
        "{}",
        preview.stderr
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_is_not_a_regular_file() {
    let dir = init("symlink");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t"]);
    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let link = d.join("link.png");
    std::os::unix::fs::symlink(&shot, &link).unwrap();
    let md = format!("![x]({})", link.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(
        run.json_error()["message"]
            .as_str()
            .unwrap()
            .contains("regular"),
        "{run:#?}"
    );
}

/// The name is the hash of the bytes stored: a file still being written while
/// `desc` copies it is named by what was copied, never by an earlier read.
#[test]
fn a_file_that_grows_while_it_is_stored_is_named_by_the_bytes_stored() {
    let dir = init("growing");
    let d = dir.path();
    ok(d, HUMAN, &["add", "clip"]);
    let clip = d.join("clip.mp4");
    video(&clip, 2 * MIB, 1);

    let done = Arc::new(AtomicBool::new(false));
    let writer = {
        let (clip, done) = (clip.clone(), Arc::clone(&done));
        thread::spawn(move || {
            let mut file = fs::OpenOptions::new().append(true).open(&clip).unwrap();
            let chunk = vec![2u8; 32 * 1024];
            let mut len = 2 * MIB;
            while !done.load(Ordering::Relaxed) && len < 24 * MIB {
                file.write_all(&chunk).unwrap();
                len += chunk.len();
                thread::sleep(Duration::from_millis(2));
            }
        })
    };
    let md = format!("![the clip]({})", clip.display());
    let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
    done.store(true, Ordering::Relaxed);
    writer.join().unwrap();
    assert_eq!(run.code, 0, "{run:#?}");

    let stored = stored_files(d);
    assert_eq!(stored.len(), 1, "{stored:?}");
    let name = stored[0].file_stem().unwrap().to_str().unwrap().to_string();
    assert_eq!(
        sha256_hex(&fs::read(&stored[0]).unwrap()),
        name,
        "the stored bytes hash to the file's name"
    );
    assert!(run.json()["body"].as_str().unwrap().contains(&name));
    assert!(temporary_files(d).is_empty(), "{:?}", temporary_files(d));
}

/// Writers storing the same new file at once: one copy is kept, and every
/// other writer's temporary copy is removed.
#[test]
fn writers_storing_the_same_file_at_once_leave_no_temporary_copies() {
    let dir = init("same-file");
    let d = dir.path();
    let clip = d.join("clip.mp4");
    video(&clip, 12 * MIB, 3);
    for title in ["a", "b", "c"] {
        ok(d, HUMAN, &["add", title]);
    }
    let md = format!("![the clip]({})", clip.display());
    let writers: Vec<_> = [("1", CLAUDE), ("2", CODEX), ("3", HUMAN)]
        .into_iter()
        .map(|(id, who)| {
            cmd(d, who, &["desc", id, &md])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut writer in writers {
        assert!(writer.wait().unwrap().success());
    }
    assert!(temporary_files(d).is_empty(), "{:?}", temporary_files(d));
    assert_eq!(stored_files(d).len(), 1, "{:?}", stored_files(d));
}

/// Export copies and checks files before it takes the ledger lock, so a big
/// video doesn't hold up other writers (ADR-008).
#[test]
fn export_copies_files_without_holding_the_ledger_lock() {
    let dir = init("export-lock");
    let d = dir.path();
    let clip = d.join("clip.mp4");
    video(&clip, 16 * MIB, 4);
    let md = format!("![the clip]({})", clip.display());
    ok(d, HUMAN, &["add", "demo", "--body", &md]);

    let out = d.join("out");
    fs::create_dir(&out).unwrap();
    let csv = out.join("tasks.csv");
    let mut export = cmd(
        d,
        HUMAN,
        &["export", "notion", "--out", csv.to_str().unwrap()],
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
    // Another writer keeps taking the lock while the export runs. Holding the
    // lock while hashing and copying, the export made it wait most of the
    // run; holding it only to record, a small part of it. Comparing the two
    // keeps this independent of how fast the machine is.
    let started = Instant::now();
    let store = Store::new(d).with_lock_timeout(Duration::from_secs(30));
    let mut probes = 0;
    let mut longest_wait = Duration::ZERO;
    let status = loop {
        if let Some(status) = export.try_wait().unwrap() {
            break status;
        }
        let asked = Instant::now();
        let tx = store.begin();
        longest_wait = longest_wait.max(asked.elapsed());
        assert!(tx.is_ok(), "{:?}", tx.err());
        drop(tx);
        probes += 1;
        thread::sleep(Duration::from_millis(5));
    };
    let run = started.elapsed();
    assert!(status.success());
    assert!(probes > 0, "the export finished before any probe ran");
    assert!(
        longest_wait * 3 < run,
        "another writer waited {longest_wait:?} for the lock during a {run:?} export"
    );
    assert!(csv.exists());
    assert_eq!(fs::read_dir(out.join("media")).unwrap().count(), 1);
}

/// Search sees a link's URL — a PR or an issue is what you search for before
/// filing a duplicate — but not a store file's name.
#[test]
fn search_matches_a_link_target_but_not_a_store_file() {
    let dir = init("search-links");
    let d = dir.path();
    let md = "Fix tracked in [the PR](https://github.com/acme/app/pull/4821).";
    ok(d, HUMAN, &["add", "token refresh", "--body", md]);
    let found = ok(d, HUMAN, &["list", "--search", "pull/4821", "--json"]).json();
    assert_eq!(found.as_array().unwrap().len(), 1, "{found}");

    let shot = d.join("shot.png");
    fs::write(&shot, PNG).unwrap();
    let md = format!("![the 401]({})", shot.display());
    ok(d, HUMAN, &["add", "bug", "--body", &md]);
    let name = stored_files(d)[0]
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let linked = format!("the raw [file](media/{name})");
    ok(d, HUMAN, &["add", "plain link", "--body", &linked]);
    let media = ok(d, HUMAN, &["list", "--search", "media/", "--json"]).json();
    assert_eq!(media.as_array().unwrap().len(), 0, "{media}");
}

/// Only an inline link — `![caption](path)` — is copied in. A reference-style
/// image of a local file would keep a path no other machine has: refused.
#[test]
fn a_reference_style_image_of_a_local_file_is_refused() {
    let dir = init("reference");
    let d = dir.path();
    ok(d, HUMAN, &["add", "t", "--body", "before"]);
    let shot = d.join("ref.png");
    fs::write(&shot, PNG).unwrap();
    let def = format!("[s]: <{}>", shot.display());
    for image in ["![shot][s]", "![s][]", "![s]"] {
        let md = format!("See {image}.\n\n{def}");
        let run = tasks(d, HUMAN, &["desc", "1", &md, "--json"]);
        assert_eq!(run.code, 2, "{image}: {run:#?}");
        let message = run.json_error()["message"].as_str().unwrap().to_string();
        assert!(message.contains("inline"), "{image}: {message}");
        assert_eq!(body_of(d, "1"), "before", "{image}: nothing was written");
        assert!(stored_files(d).is_empty(), "{image}: nothing was copied");
    }

    let url = "See ![logo][l].\n\n[l]: https://example.com/logo.png";
    ok(d, HUMAN, &["desc", "1", url]);
    assert_eq!(body_of(d, "1"), url, "a reference to a URL is kept as text");
}
