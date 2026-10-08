//! An explicitly launched, loopback-only browser interface. The HTTP layer
//! translates typed requests into library operations; it never writes a ledger.

mod markdown;

use crate::error::{Error, Result};
use crate::model::{Priority, State};
use crate::ops::{self, Changes, Ctx, Destination, ExportSelection, Filter, NewTask};
use crate::render::{DetailView, ErrorView, ExportView, FieldsView, FormatView, TaskView};
use crate::store::Store;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use tiny_http::{Header, Method, Request, Response, Server};
use ulid::Ulid;

const MAX_BODY: u64 = 2 * 1024 * 1024;
const HTML: &str = include_str!("../ui/index.html");
const CSS: &str = include_str!("../ui/style.css");
const JS: &str = include_str!("../ui/app.js");

/// Start a UI for an already discovered store. The token is in the URL
/// fragment, so it is not sent in request URLs or referrers.
pub fn run(
    folder: &Path,
    port: u16,
    no_open: bool,
    json_output: bool,
    out: &mut impl Write,
) -> Result<()> {
    let server = Server::http(("127.0.0.1", port)).map_err(|e| {
        Error::io(
            "couldn't start the local UI",
            std::io::Error::other(e.to_string()),
        )
    })?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| Error::Usage("the UI needs a loopback TCP address".into()))?;
    let origin = format!("http://{address}");
    let token = format!("{}{}", Ulid::new(), Ulid::new());
    let url = format!("{origin}/#{token}");
    let warnings = Rc::new(RefCell::new(Vec::<String>::new()));
    let captured = warnings.clone();
    let store = Store::in_folder(folder).on_warning(move |warning| {
        eprintln!("warning: {warning}");
        captured.borrow_mut().push(warning.into());
    });
    // Validate before advertising a working UI. This never initializes a store.
    store.read()?;
    store.config()?;
    if json_output {
        writeln!(
            out,
            "{}",
            json!({"url":url,"store":folder,"actor":"human:local"})
        )
        .map_err(|e| Error::io("couldn't print UI URL", e))?;
    } else {
        writeln!(
            out,
            "HippoTask → {url}\nStore: {}\nPress Ctrl-C to stop.",
            folder.display()
        )
        .map_err(|e| Error::io("couldn't print UI URL", e))?;
    }
    out.flush()
        .map_err(|e| Error::io("couldn't flush UI URL", e))?;
    if !no_open {
        if let Err(error) = open_browser(&url) {
            eprintln!("warning: {error}; open the printed URL manually");
        }
    }
    let ui = App {
        store,
        origin,
        token,
        node: format!("ui-{}", Ulid::new()),
        warnings,
    };
    loop {
        let mut request = server
            .recv()
            .map_err(|e| Error::io("couldn't receive UI request", e))?;
        ui.warnings.borrow_mut().clear();
        let (code, body, content_type) = ui.response(&mut request);
        let mut response = Response::from_data(body).with_status_code(code);
        for (name, value) in [
            ("Content-Type", content_type),
            ("Cache-Control", "no-store"),
            ("X-Content-Type-Options", "nosniff"),
            ("Referrer-Policy", "no-referrer"),
            ("Content-Security-Policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' blob:; media-src blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"),
        ] {
            let header = Header::from_bytes(name, value)
                .map_err(|_| Error::Usage(format!("invalid UI response header: {name}")))?;
            response.add_header(header);
        }
        if let Err(error) = request.respond(response) {
            eprintln!("warning: couldn't send UI response: {error}");
        }
    }
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let result = Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = Command::new("xdg-open").arg(url).status();
    let status = result.map_err(|e| Error::io("couldn't open the browser", e))?;
    if !status.success() {
        return Err(Error::Usage(format!("browser opener exited with {status}")));
    }
    Ok(())
}

struct App {
    store: Store,
    origin: String,
    token: String,
    node: String,
    warnings: Rc<RefCell<Vec<String>>>,
}

impl App {
    fn response(&self, request: &mut Request) -> (u16, Vec<u8>, &'static str) {
        let host = self.origin.trim_start_matches("http://");
        if header(request, "Host") != Some(host) {
            return denied("unexpected host");
        }
        let path = request.url().to_string();
        if request.method() == &Method::Get {
            let asset = match path.as_str() {
                "/" => Some((HTML, "text/html; charset=utf-8")),
                "/style.css" => Some((CSS, "text/css; charset=utf-8")),
                "/app.js" => Some((JS, "text/javascript; charset=utf-8")),
                _ => None,
            };
            if let Some((body, content_type)) = asset {
                return (200, body.as_bytes().to_vec(), content_type);
            }
        }
        if header(request, "X-Hippo-Token") != Some(self.token.as_str()) {
            return denied(
                "this UI session is unavailable — reopen the URL printed in your terminal",
            );
        }
        let origin = header(request, "Origin");
        if origin.is_some_and(|origin| origin != self.origin)
            || (request.method() != &Method::Get && origin != Some(self.origin.as_str()))
        {
            return denied("unexpected request origin");
        }
        let result = if request.method() == &Method::Get && path.starts_with("/api/media/") {
            match crate::media::read_preview(&self.store, &path["/api/media/".len()..]) {
                Ok((bytes, mime)) => return (200, bytes, mime),
                Err(error) => Err(error),
            }
        } else {
            self.api(request, &path)
        };
        let (code, mut value) = match result {
            Ok(value) => (200, value),
            Err(error) => {
                let code = match &error {
                    Error::Io { .. } => 500,
                    Error::Usage(_) => 400,
                    Error::NotFound(_) => 404,
                    Error::Conflict(_) | Error::Stale(_) => 409,
                };
                let mut value = json!(ErrorView::new(&error));
                if path.starts_with("/api/media/") && matches!(error, Error::NotFound(_)) {
                    value["message"] = json!("This media file is missing from the store.");
                }
                (code, value)
            }
        };
        if let Some(object) = value.as_object_mut() {
            object.insert("warnings".into(), json!(*self.warnings.borrow()));
        }
        (
            code,
            value.to_string().into_bytes(),
            "application/json; charset=utf-8",
        )
    }

    /// A saved task, plus what it still misses from the project's format
    /// (ADR-012), in words the editor can show: `kind`, `name`, `label`, and
    /// `message` — the CLI's text, which is also among the warnings, so the
    /// page can keep it out of its banner. The save already happened, so a
    /// config that can't be read is a warning (worded as `ops` words it, so
    /// the page shows it once), never a failed save.
    fn saved(&self, task: &crate::model::Task, now: i64) -> Value {
        let mut value = json!(TaskView::new(task, now, self.store.folder()));
        let gaps: Vec<Value> = match crate::config::Config::load(self.store.folder()) {
            Ok(config) => crate::format::gaps(&config, task)
                .iter()
                .map(|gap| {
                    json!({
                        "kind": gap.kind(),
                        "name": gap.name(),
                        "label": gap.label(),
                        "message": gap.message(task),
                    })
                })
                .collect(),
            Err(e) => {
                self.store.warn(&format!(
                    "couldn't check {} against this project's task format: {e}",
                    task.handle()
                ));
                Vec::new()
            }
        };
        if let Some(object) = value.as_object_mut() {
            object.insert("format_gaps".into(), Value::Array(gaps));
        }
        value
    }

    fn api(&self, request: &mut Request, path: &str) -> Result<Value> {
        if request.method() == &Method::Post && path == "/api/media" {
            if header(request, "Content-Type") != Some("application/octet-stream") {
                return Err(Error::Usage(
                    "send media bytes as application/octet-stream".into(),
                ));
            }
            let limits = self.store.config()?.media;
            let length = request.body_length().map(|len| len as u64);
            let file =
                crate::media::upload(&self.store, &mut request.as_reader(), length, &limits)?;
            return Ok(json!({"path":file.path,"mime":file.mime,"bytes":file.bytes}));
        }
        if request.method() == &Method::Post && path == "/api/markdown/preview" {
            let draft: Markdown = read_json(request)?;
            return Ok(json!({"html": markdown::render(&draft.markdown)}));
        }
        // Do not inherit the launching agent's identity or claim node.
        let now = chrono::Utc::now().timestamp_millis();
        let ctx = Ctx::new(None, Some(self.node.clone()), now)?;
        if request.method() == &Method::Get && path == "/api/state" {
            let tasks = ops::list(&self.store, &ctx, &Filter::default())?;
            let fields = ops::fields(&self.store)?;
            let changed = ops::changed_since_export(&self.store, Destination::Notion)?;
            let limits = self.store.config()?.media;
            return Ok(json!({
                "tasks": tasks.iter().map(|t| TaskView::new(t, now, self.store.folder())).collect::<Vec<_>>(),
                "fields": FieldsView::new(&fields),
                "format": FormatView::new(&fields.config),
                "store": self.store.folder(), "actor": ctx.actor,
                "changed": changed.iter().map(|t| &t.id).collect::<Vec<_>>(),
                "media_limits": {"max_image_bytes":limits.max_image_bytes,"max_video_bytes":limits.max_video_bytes}
            }));
        }
        if let Some(id) = path.strip_prefix("/api/task/") {
            if request.method() == &Method::Get {
                let detail = ops::show(&self.store, id)?;
                return Ok(json!(DetailView::new(&detail, now, self.store.folder())));
            }
            if request.method() == &Method::Patch {
                let edit: Edit = read_json(request)?;
                let changes = Changes {
                    title: edit.title,
                    body: edit.body,
                    state: edit.state,
                    priority: edit.priority,
                    assignee: edit.assignee,
                    unassign: edit.unassign,
                    label_add: edit.label_add,
                    label_remove: edit.label_remove,
                    fields: edit.fields.into_iter().collect(),
                    clear_fields: edit.clear_fields,
                    ..Changes::default()
                };
                let task = ops::update_checked(&self.store, &ctx, id, changes, edit.base)?;
                return Ok(self.saved(&task, now));
            }
        }
        if request.method() == &Method::Post && path == "/api/tasks" {
            let new: Create = read_json(request)?;
            let task = ops::add(
                &self.store,
                &ctx,
                NewTask {
                    title: new.title,
                    priority: new.priority,
                    body: new.body,
                    assignee: new.assignee,
                    labels: new.labels,
                    fields: new.fields.into_iter().collect(),
                },
            )?;
            return Ok(self.saved(&task, now));
        }
        if request.method() == &Method::Post
            && matches!(path, "/api/export/preview" | "/api/export/save")
        {
            let export: Export = read_json(request)?;
            if export.ids.is_empty() {
                return Err(Error::Usage("select at least one task to export".into()));
            }
            let selection = ExportSelection {
                ids: Some(export.ids),
                all: true,
                again: export.again,
                ..ExportSelection::default()
            };
            if path == "/api/export/preview" {
                let preview =
                    ops::export(&self.store, &ctx, Destination::Notion, &selection, None)?;
                return Ok(json!({
                    "review": preview.review, "csv": preview.csv,
                    "exported": preview.tasks.iter().map(|t| TaskView::new(t, now, self.store.folder())).collect::<Vec<_>>(),
                    "changed": preview.changed.iter().map(|t| TaskView::new(t, now, self.store.folder())).collect::<Vec<_>>(),
                    "suggested_path": self.store.folder().join(format!("notion-{}.csv", Ulid::new()))
                }));
            }
            let path = export
                .path
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or_else(|| Error::Usage("choose a CSV file path".into()))?;
            let path = if path.is_absolute() {
                path
            } else {
                self.store.folder().join(path)
            };
            if path
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("csv"))
            {
                return Err(Error::Usage("the export file must end in .csv".into()));
            }
            let review = export
                .review
                .ok_or_else(|| Error::Usage("preview this export first".into()))?;
            let saved = ops::export_reviewed(
                &self.store,
                &ctx,
                Destination::Notion,
                &selection,
                &path,
                &review,
            )?;
            return Ok(json!(ExportView::new(&saved, now, self.store.folder())));
        }
        Err(Error::NotFound(path.into()))
    }
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn denied(message: &str) -> (u16, Vec<u8>, &'static str) {
    (
        403,
        json!({"error":"forbidden","message":message})
            .to_string()
            .into_bytes(),
        "application/json; charset=utf-8",
    )
}

fn read_json<T: serde::de::DeserializeOwned>(request: &mut Request) -> Result<T> {
    if header(request, "Content-Type")
        .is_none_or(|v| v.split(';').next() != Some("application/json"))
    {
        return Err(Error::Usage("send application/json".into()));
    }
    if request.body_length().is_some_and(|n| n as u64 > MAX_BODY) {
        return Err(Error::Usage("request is too large (maximum 2 MiB)".into()));
    }
    let mut body = Vec::new();
    request
        .as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|e| Error::io("couldn't read UI request", e))?;
    if body.len() as u64 > MAX_BODY {
        return Err(Error::Usage("request is too large (maximum 2 MiB)".into()));
    }
    serde_json::from_slice(&body).map_err(|e| Error::Usage(format!("invalid request: {e}")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Markdown {
    markdown: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    base: u64,
    title: Option<String>,
    body: Option<String>,
    state: Option<State>,
    priority: Option<Priority>,
    assignee: Option<String>,
    #[serde(default)]
    unassign: bool,
    #[serde(default)]
    label_add: Vec<String>,
    #[serde(default)]
    label_remove: Vec<String>,
    #[serde(default)]
    fields: BTreeMap<String, String>,
    #[serde(default)]
    clear_fields: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    title: String,
    #[serde(default = "no_priority")]
    priority: Priority,
    body: Option<String>,
    assignee: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    fields: BTreeMap<String, String>,
}

fn no_priority() -> Priority {
    Priority::None
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    ids: Vec<String>,
    #[serde(default)]
    again: bool,
    review: Option<String>,
    path: Option<PathBuf>,
}
