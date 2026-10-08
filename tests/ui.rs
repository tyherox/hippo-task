//! Exercise the shipped HTTP boundary through the real CLI process.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::{bare, ok, TempDir, HUMAN};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Stdio};
use std::time::Duration;

struct Ui {
    _child: Process,
    address: String,
    origin: String,
    token: String,
}

// Own the child immediately, so even a failed startup assertion stops it.
struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Ui {
    fn start(dir: &TempDir) -> Self {
        let child = bare()
            .current_dir(dir.path())
            .args(["--dir", ".", "ui", "--no-open", "--json"])
            .env("HIPPO_ACTOR", "agent:inherited")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut child = Process(child);
        let mut line = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let ready: Value = serde_json::from_str(&line).expect("UI prints its launch URL as JSON");
        assert_eq!(ready["actor"], "human:local");
        assert!(std::path::Path::new(ready["store"].as_str().unwrap()).is_absolute());
        let url = ready["url"].as_str().unwrap();
        let (origin, token) = url.split_once("/#").unwrap();
        Self {
            _child: child,
            address: origin.trim_start_matches("http://").into(),
            origin: origin.into(),
            token: token.into(),
        }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        authorized: bool,
        origin: &str,
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        let token = if authorized {
            format!("X-Hippo-Token: {}\r\n", self.token)
        } else {
            String::new()
        };
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: {}\r\nOrigin: {origin}\r\n{token}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", self.address, body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let code = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        (code, body.into())
    }

    fn api(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let (code, body) = self.request(method, path, body, true, &self.origin);
        (code, serde_json::from_str(&body).expect("JSON response"))
    }

    fn upload(&self, bytes: &[u8], authorized: bool, origin: &str) -> (u16, Value) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let token = if authorized {
            format!("X-Hippo-Token: {}\r\n", self.token)
        } else {
            String::new()
        };
        write!(stream, "POST /api/media HTTP/1.1\r\nHost: {}\r\nOrigin: {origin}\r\n{token}Content-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", self.address, bytes.len()).unwrap();
        stream.write_all(bytes).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        (
            headers.split_whitespace().nth(1).unwrap().parse().unwrap(),
            serde_json::from_str(body).unwrap(),
        )
    }
}

#[test]
fn ui_launch_is_read_only_and_api_requires_its_session_and_origin() {
    let dir = TempDir::new("ui-boundary");
    ok(dir.path(), HUMAN, &["add", "Real task"]);
    let before = dir.ledger_text();
    let ui = Ui::start(&dir);
    let (code, html) = ui.request("GET", "/", None, false, &ui.origin);
    assert_eq!(code, 200);
    assert!(html.contains("HippoTask"));
    assert_eq!(
        ui.request("GET", "/api/state", None, false, &ui.origin).0,
        403
    );
    assert_eq!(
        ui.request(
            "POST",
            "/api/tasks",
            Some(json!({"title":"Attack"})),
            true,
            "https://example.com"
        )
        .0,
        403
    );
    assert_eq!(ui.api("POST", "/reset", Some(json!({}))).0, 404);
    let (code, state) = ui.api("GET", "/api/state", None);
    assert_eq!(code, 200);
    assert_eq!(state["tasks"][0]["title"], "Real task");
    assert_eq!(dir.ledger_text(), before);
}

#[test]
fn ui_edits_and_exports_real_tasks_and_reports_stale_drafts() {
    let dir = TempDir::new("ui-edit-export");
    ok(dir.path(), HUMAN, &["add", "Original"]);
    let ui = Ui::start(&dir);
    let (_, state) = ui.api("GET", "/api/state", None);
    let id = state["tasks"][0]["id"].as_str().unwrap();
    let seq = state["tasks"][0]["seq"].as_u64().unwrap();
    let path = format!("/api/task/{id}");
    let (code, edited) = ui.api(
        "PATCH",
        &path,
        Some(json!({"base":seq,"title":"Reviewed title","priority":"high"})),
    );
    assert_eq!(code, 200);
    assert_eq!(edited["title"], "Reviewed title");
    let before = dir.ledger_text();
    let (code, stale) = ui.api(
        "PATCH",
        &path,
        Some(json!({"base":seq,"title":"Old draft"})),
    );
    assert_eq!(code, 409);
    assert_eq!(stale["error"], "stale");
    assert_eq!(dir.ledger_text(), before);
    let (code, preview) = ui.api(
        "POST",
        "/api/export/preview",
        Some(json!({"ids":[id],"again":false})),
    );
    assert_eq!(code, 200);
    assert!(preview["csv"].as_str().unwrap().contains("Reviewed title"));
    assert_eq!(dir.ledger_text(), before);
    let csv = dir.path().join("review.csv");
    let (code, saved) = ui.api(
        "POST",
        "/api/export/save",
        Some(json!({"ids":[id],"again":false,"review":preview["review"],"path":csv})),
    );
    assert_eq!(code, 200);
    assert_eq!(saved["exported"].as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::read_to_string(csv).unwrap(),
        preview["csv"].as_str().unwrap()
    );
    let detail = ok(dir.path(), HUMAN, &["show", "1", "--json"]).json();
    assert_eq!(detail["title"], "Reviewed title");
    assert!(detail["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["actor"] == "human:local"));
}

#[test]
fn ui_does_not_initialize_a_missing_store() {
    let dir = TempDir::new("ui-missing");
    let result = common::tasks(dir.path(), HUMAN, &["ui", "--no-open", "--json"]);
    assert_eq!(result.code, 2);
    assert!(result.json_error()["message"]
        .as_str()
        .unwrap()
        .contains("no task store"));
    assert!(!dir.path().join(".hippotask").exists());
}

#[test]
fn description_preview_renders_unsaved_markdown_without_writing() {
    let dir = TempDir::new("ui-markdown");
    ok(
        dir.path(),
        HUMAN,
        &["add", "Saved task", "--body", "Saved description"],
    );
    let before = dir.ledger_text();
    let ui = Ui::start(&dir);
    let body = json!({"markdown":"## Draft heading\n\n**Bold** and *italic* and ~~removed~~.\n\n- [x] Reviewed\n- [ ] Pending\n\n> A quote\n\n```rust\nlet x = \"<tag>\";\n```\n\n| Name | Value |\n| --- | --- |\n| One | Two |\n\n[Docs](https://example.com/docs)"});
    assert_eq!(
        ui.request(
            "POST",
            "/api/markdown/preview",
            Some(body.clone()),
            false,
            &ui.origin
        )
        .0,
        403
    );
    assert_eq!(
        ui.request(
            "POST",
            "/api/markdown/preview",
            Some(body.clone()),
            true,
            "https://example.com"
        )
        .0,
        403
    );
    let (code, preview) = ui.api("POST", "/api/markdown/preview", Some(body));
    assert_eq!(code, 200);
    let html = preview["html"].as_str().unwrap();
    for fragment in [
        "<h2>Draft heading</h2>",
        "<strong>Bold</strong>",
        "<em>italic</em>",
        "<del>removed</del>",
        "type=\"checkbox\"",
        "disabled=\"\"",
        "<blockquote>",
        "<pre><code class=\"language-rust\">",
        "&lt;tag&gt;",
        "<table>",
        "<td>Two</td>",
        "href=\"https://example.com/docs\"",
        "rel=\"noopener noreferrer\"",
    ] {
        assert!(html.contains(fragment), "missing {fragment}: {html}");
    }
    let (code, blank) = ui.api(
        "POST",
        "/api/markdown/preview",
        Some(json!({"markdown":""})),
    );
    assert_eq!(code, 200);
    assert_eq!(blank["html"], "");
    assert_eq!(
        ui.api(
            "POST",
            "/api/markdown/preview",
            Some(json!({"markdown":null}))
        )
        .0,
        400
    );
    assert_eq!(dir.ledger_text(), before);
}

#[test]
fn description_preview_escapes_html_and_blocks_unsafe_links_and_remote_images() {
    let dir = TempDir::new("ui-markdown-safety");
    ok(dir.path(), HUMAN, &["add", "Task"]);
    let ui = Ui::start(&dir);
    let (_, preview) = ui.api("POST", "/api/markdown/preview", Some(json!({"markdown":r#"<script>alert(1)</script>

<img src=x onerror=alert(1)>

[bad](javascript:alert%281%29) [encoded](jav&#x61;script:alert%281%29) [data](data:text/html,hi) [file](file:///etc/passwd) [relative](/api/state) [protocol](//example.com)

![Tracker](https://example.com/tracker.png) ![attack](javascript:alert%281%29)

[mail](mailto:test@example.com) [safe](HTTPS://example.com "a & b")
"#})));
    let html = preview["html"].as_str().unwrap();
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("<img"));
    // Raw HTML is escaped and visible as text; only actual tags must lack src.
    assert!(!html
        .split('<')
        .skip(1)
        .filter_map(|part| part.split_once('>'))
        .any(|(tag, _)| tag.contains("src=")));
    assert!(!html.contains("href=\"javascript:"));
    assert!(!html.contains("href=\"data:"));
    assert!(!html.contains("href=\"file:"));
    assert!(!html.contains("href=\"/"));
    assert!(html.contains("Tracker"));
    assert!(html.contains("External image not loaded"));
    assert!(html.contains("href=\"mailto:test@example.com\""));
    assert!(html.contains("href=\"HTTPS://example.com\""));
    let (_, nested) = ui.api("POST", "/api/markdown/preview", Some(json!({"markdown":format!("[![&quot; onerror=&quot;bad **caption**](media/{}.png)](javascript:bad)\n\n![![nested](https://example.com/nested.png)](https://example.com/outer.png)", "a".repeat(64))})));
    let nested = nested["html"].as_str().unwrap();
    assert!(nested.contains("data-caption=\"&quot; onerror=&quot;bad caption\""));
    assert!(!nested.contains("<img"));
    assert!(!nested.contains("href="));
    assert!(!nested.contains(" onerror=\""));
}

#[test]
fn preview_media_is_authenticated_bounded_and_confined_to_the_store() {
    use sha2::{Digest, Sha256};
    let dir = TempDir::new("ui-preview-media");
    ok(dir.path(), HUMAN, &["add", "Task"]);
    let media = dir.path().join(".hippotask/media");
    std::fs::create_dir_all(&media).unwrap();
    // ASCII fixture keeps the existing HTTP helper usable; the reader validates
    // file signatures and hashes, while the browser owns image decoding.
    let bytes = b"GIF89a fixture";
    let name = format!("{:x}.gif", Sha256::digest(bytes));
    std::fs::write(media.join(&name), bytes).unwrap();
    let ui = Ui::start(&dir);
    let before = dir.ledger_text();
    let (code, preview) = ui.api(
        "POST",
        "/api/markdown/preview",
        Some(json!({"markdown":format!("![A **stored** image](media/{name})")})),
    );
    assert_eq!(code, 200);
    let html = preview["html"].as_str().unwrap();
    assert!(html.contains(&format!("data-media=\"media/{name}\"")));
    assert!(html.contains("A stored image"));
    let route = format!("/api/media/{name}");
    assert_eq!(ui.request("GET", &route, None, false, &ui.origin).0, 403);
    assert_eq!(
        ui.request("GET", &route, None, true, "https://example.com")
            .0,
        403
    );
    let (code, data) = ui.request("GET", &route, None, true, &ui.origin);
    assert_eq!(code, 200);
    assert_eq!(data.as_bytes(), bytes);
    for path in [
        "/api/media/../ledger.jsonl",
        "/api/media/%2e%2e%2fledger.jsonl",
        "/api/media/ledger.jsonl",
    ] {
        assert_eq!(ui.api("GET", path, None).0, 400, "{path}");
    }
    std::fs::write(media.join(&name), b"<html>not an image</html>").unwrap();
    assert_eq!(ui.api("GET", &route, None).0, 400);
    std::fs::write(media.join(&name), b"GIF89a changed bytes").unwrap();
    assert_eq!(ui.api("GET", &route, None).0, 400);
    let oversized = std::fs::File::create(media.join(&name)).unwrap();
    oversized.set_len(8 * 1024 * 1024 + 1).unwrap();
    // GIF gets the image cap, even when the configured video cap is larger.
    std::fs::OpenOptions::new()
        .write(true)
        .open(media.join(&name))
        .unwrap()
        .write_all(b"GIF89a")
        .unwrap();
    assert_eq!(ui.api("GET", &route, None).0, 400);
    std::fs::remove_file(media.join(&name)).unwrap();
    assert_eq!(ui.api("GET", &route, None).0, 404);
    assert_eq!(
        ui.api("GET", &route, None).1["message"],
        "This media file is missing from the store."
    );
    #[cfg(unix)]
    {
        let outside = dir.path().join("outside.gif");
        std::fs::write(&outside, bytes).unwrap();
        std::os::unix::fs::symlink(&outside, media.join(&name)).unwrap();
        assert_eq!(ui.api("GET", &route, None).0, 400);
        std::fs::remove_file(media.join(&name)).unwrap();
        std::fs::remove_dir(&media).unwrap();
        let outside_dir = dir.path().join("outside-media");
        std::fs::create_dir(&outside_dir).unwrap();
        std::fs::write(outside_dir.join(&name), bytes).unwrap();
        std::os::unix::fs::symlink(outside_dir, &media).unwrap();
        assert_eq!(ui.api("GET", &route, None).0, 400);
    }
    assert_eq!(dir.ledger_text(), before);
}

#[test]
fn ui_uploads_deduplicated_media_without_saving_a_task_until_requested() {
    use sha2::{Digest, Sha256};
    let dir = TempDir::new("ui-upload");
    ok(dir.path(), HUMAN, &["add", "Task"]);
    let ui = Ui::start(&dir);
    let before = dir.ledger_text();
    let bytes = b"GIF89a upload fixture";
    let (code, upload) = ui.upload(bytes, true, &ui.origin);
    assert_eq!(code, 200);
    let path = format!("media/{:x}.gif", Sha256::digest(bytes));
    assert_eq!(upload["path"], path);
    assert_eq!(upload["mime"], "image/gif");
    assert_eq!(upload["bytes"], bytes.len());
    assert_eq!(
        std::fs::read(dir.path().join(".hippotask").join(&path)).unwrap(),
        bytes
    );
    assert_eq!(ui.upload(bytes, true, &ui.origin).1["path"], path);
    assert_eq!(
        std::fs::read_dir(dir.path().join(".hippotask/media"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(dir.ledger_text(), before);
    let (_, state) = ui.api("GET", "/api/state", None);
    assert_eq!(state["media_limits"]["max_image_bytes"], 8 * 1024 * 1024);
    assert_eq!(state["media_limits"]["max_video_bytes"], 64 * 1024 * 1024);
    let task = &state["tasks"][0];
    assert!(task["body"].is_null());
    let route = format!("/api/task/{}", task["id"].as_str().unwrap());
    let markdown = format!("An image\n\n![Image]({path})");
    let (code, saved) = ui.api(
        "PATCH",
        &route,
        Some(json!({"base":task["seq"],"body":markdown})),
    );
    assert_eq!(code, 200);
    assert_eq!(saved["body"], markdown);
    assert_eq!(saved["media"][0]["bytes"], bytes.len());
    let mut cli = ok(dir.path(), HUMAN, &["show", "1", "--json"]).json();
    assert_eq!(
        std::fs::canonicalize(cli["media"][0]["path"].as_str().unwrap()).unwrap(),
        std::fs::canonicalize(saved["media"][0]["path"].as_str().unwrap()).unwrap()
    );
    // macOS exposes the same temp folder through /var and /private/var.
    cli["media"][0]["path"] = saved["media"][0]["path"].clone();
    assert_eq!(cli["media"], saved["media"]);
    let csv = dir.path().join("out/tasks.csv");
    std::fs::create_dir(csv.parent().unwrap()).unwrap();
    ok(
        dir.path(),
        HUMAN,
        &["export", "notion", "--out", csv.to_str().unwrap(), "--json"],
    );
    assert_eq!(
        std::fs::read(csv.parent().unwrap().join(&path)).unwrap(),
        bytes
    );
    assert!(std::fs::read_to_string(csv).unwrap().contains(&markdown));
}

#[test]
fn ui_upload_refuses_unauthorized_invalid_and_oversized_files_without_leaving_files() {
    let dir = TempDir::new("ui-upload-refusals");
    ok(dir.path(), HUMAN, &["add", "Task"]);
    std::fs::write(
        dir.path().join(".hippotask/config.toml"),
        "[media]\nmax_image_mib = 1\nmax_video_mib = 2\n",
    )
    .unwrap();
    let ui = Ui::start(&dir);
    let before = dir.ledger_text();
    assert_eq!(ui.upload(b"GIF89a", false, &ui.origin).0, 403);
    assert_eq!(ui.upload(b"GIF89a", true, "https://example.com").0, 403);
    for bytes in [
        &b""[..],
        b"<svg onload=alert(1)></svg>",
        b"some file.png",
        b"<html>bad</html>",
    ] {
        assert_eq!(ui.upload(bytes, true, &ui.origin).0, 400);
    }
    assert_eq!(
        ui.api("POST", "/api/media", Some(json!({"path":"/etc/passwd"})))
            .0,
        400
    );
    let mut over = vec![0; 1024 * 1024 + 1];
    over[..6].copy_from_slice(b"GIF89a");
    let (code, error) = ui.upload(&over, true, &ui.origin);
    assert_eq!(code, 400);
    assert!(error["message"].as_str().unwrap().contains("limit"));
    let media = dir.path().join(".hippotask/media");
    assert!(!media.exists() || std::fs::read_dir(&media).unwrap().count() == 0);
    // A valid MP4 uses the video cap, despite having the same size as the refused GIF.
    let mp4_header = b"\x00\x00\x00\x18ftypisom\x00\x00\x00\x00isommp42";
    over[..mp4_header.len()].copy_from_slice(mp4_header);
    let (code, video) = ui.upload(&over, true, &ui.origin);
    assert_eq!(code, 200);
    assert_eq!(video["mime"], "video/mp4");
    assert_eq!(video["bytes"], over.len());
    assert_eq!(std::fs::read_dir(&media).unwrap().count(), 1);
    assert_eq!(dir.ledger_text(), before);
}

#[cfg(unix)]
#[test]
fn ui_upload_never_follows_store_media_symlinks_or_overwrites_damaged_files() {
    use sha2::{Digest, Sha256};
    let dir = TempDir::new("ui-upload-symlinks");
    ok(dir.path(), HUMAN, &["add", "Task"]);
    let ui = Ui::start(&dir);
    let media = dir.path().join(".hippotask/media");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &media).unwrap();
    assert_eq!(ui.upload(b"GIF89a", true, &ui.origin).0, 400);
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    std::fs::remove_file(&media).unwrap();
    std::fs::create_dir(&media).unwrap();
    let bytes = b"GIF89a";
    let dest = media.join(format!("{:x}.gif", Sha256::digest(bytes)));
    let secret = outside.join("file");
    std::fs::write(&secret, bytes).unwrap();
    std::os::unix::fs::symlink(&secret, &dest).unwrap();
    assert_eq!(ui.upload(bytes, true, &ui.origin).0, 400);
    assert_eq!(std::fs::read(&secret).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(&media).unwrap().count(), 1);
    std::fs::remove_file(&dest).unwrap();
    std::fs::write(&dest, b"damaged").unwrap();
    assert_eq!(ui.upload(bytes, true, &ui.origin).0, 400);
    assert_eq!(std::fs::read(&dest).unwrap(), b"damaged");
    assert_eq!(std::fs::read_dir(&media).unwrap().count(), 1);
}

#[test]
fn ui_state_carries_the_task_format_and_a_save_reports_its_gaps() {
    let dir = TempDir::new("ui-format");
    ok(dir.path(), HUMAN, &["add", "Existing"]);
    std::fs::write(
        dir.path().join(".hippotask").join("config.toml"),
        "[format]\ntemplate = \"## Done when\\n- [ ]\\n\"\nrequired_sections = [\"Done when\"]\n",
    )
    .unwrap();
    let ui = Ui::start(&dir);
    let (_, state) = ui.api("GET", "/api/state", None);
    assert_eq!(state["format"]["template"], "## Done when\n- [ ]\n");
    assert_eq!(state["format"]["required_sections"], json!(["Done when"]));
    let (code, saved) = ui.api(
        "POST",
        "/api/tasks",
        Some(json!({"title":"From the template","body":"## Done when\n- [ ]\n"})),
    );
    assert_eq!(code, 200, "{saved}");
    assert_eq!(saved["title"], "From the template");
    let warnings = saved["warnings"].to_string();
    assert!(warnings.contains("Done when"), "{saved}");
}
