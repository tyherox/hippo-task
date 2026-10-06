"use strict";

// One collection, one selection, and one set of drafts power both views.
// Drafts stay in this tab; only an explicit Save reaches the local ledger.
const $ = (selector) => document.querySelector(selector);
const escapeHtml = (value) => String(value ?? "").replace(/[&<>"']/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const statuses = {todo:"Todo", doing:"Doing", done:"Done", cancelled:"Cancelled"};
const priorities = {none:"No priority", low:"Low", med:"Medium", high:"High", urgent:"Urgent"};
let token = location.hash.slice(1);
try {
  if (token) sessionStorage.setItem("hippo-session", token);
  else token = sessionStorage.getItem("hippo-session") || "";
} catch (error) {
  console.warn("Session storage unavailable; use the launch URL after reloading.", error);
}
history.replaceState(null, "", "/");
const drafts = new Map();
const selected = new Set();
let tasks = [], fields = [], changed = new Set(), active = null, inline = null, view = "board";
let fieldFilters = {}, connected = false, refreshing = false, dragging = false, needsRender = true, toastTimer;
let exportIds = [], previewData = null, previewVersion = 0, exporting = false;
let descriptionRequest = null, descriptionMediaUrls = [];
let mediaPicker = null, mediaLimits = {max_image_bytes:8 * 1024 * 1024,max_video_bytes:64 * 1024 * 1024};

async function api(path, method = "GET", body, signal) {
  let response;
  try {
    response = await fetch(path, {method, signal, headers:{"X-Hippo-Token":token, ...(body ? {"Content-Type":"application/json"} : {})}, body:body ? JSON.stringify(body) : undefined});
  } catch (cause) {
    if (cause.name === "AbortError") throw cause;
    const error = new Error("The local server is unavailable. Your unsaved edits are still in this tab. Restart the UI and keep this tab open to copy them.");
    error.kind = "connection";
    throw error;
  }
  const result = await response.json();
  showWarnings(result.warnings || []);
  if (!response.ok) {
    const error = new Error(result.message || "The request failed.");
    error.kind = result.error;
    throw error;
  }
  return result;
}

function showWarnings(warnings) {
  if (warnings.length) {
    $("#warnings").textContent = [...new Set(warnings)].join("\n");
    $("#warnings").hidden = false;
  }
}

function toast(message, error = false) {
  clearTimeout(toastTimer);
  $("#toast").textContent = message;
  $("#toast").className = error ? "error" : "";
  $("#toast").hidden = false;
  toastTimer = setTimeout(() => { $("#toast").hidden = true; }, error ? 8000 : 4200);
}

function options(values, current, blank) {
  return (blank !== undefined ? `<option value="">${escapeHtml(blank)}</option>` : "") + Object.entries(values).map(([value, label]) => `<option value="${escapeHtml(value)}"${value === current ? " selected" : ""}>${escapeHtml(label)}</option>`).join("");
}
function taskById(id) { return tasks.find((task) => task.id === id); }
function exportState(task) { return changed.has(task.id) ? "changed" : task.exported.notion ? "exported" : "new"; }
function exportLabel(task) { return {new:"Not exported", exported:"Exported", changed:"Changed since export"}[exportState(task)]; }
function held(task) { return Boolean(task.lease?.active); }
function tags(text) { return [...new Set(text.split(",").map((s) => s.trim()).filter(Boolean))]; }

function newDraft(task) {
  return {base:structuredClone(task), values:{title:task.title, body:task.body || "", state:task.state, priority:task.priority, assignee:task.assignee || "", labels:task.labels.join(", "), fields:{...task.fields}}, error:"", latest:null, saving:false, descriptionMode:"write", upload:null, mediaMessage:"", mediaErrors:[]};
}
function draftFor(id) {
  if (!drafts.has(id)) drafts.set(id, newDraft(taskById(id)));
  return drafts.get(id);
}
function patchFor(draft) {
  const {base, values} = draft;
  const patch = {base:base.seq};
  for (const key of ["title", "body", "state", "priority"]) {
    if (values[key] !== (base[key] ?? "")) patch[key] = values[key];
  }
  if (values.assignee !== (base.assignee || "")) {
    if (values.assignee.trim()) patch.assignee = values.assignee.trim();
    else patch.unassign = true;
  }
  if (values.labels !== base.labels.join(", ")) {
    const next = tags(values.labels);
    patch.label_add = next.filter((tag) => !base.labels.includes(tag));
    patch.label_remove = base.labels.filter((tag) => !next.includes(tag));
    if (!patch.label_add.length) delete patch.label_add;
    if (!patch.label_remove.length) delete patch.label_remove;
  }
  for (const key of new Set([...Object.keys(base.fields), ...Object.keys(values.fields)])) {
    const value = values.fields[key] || "";
    if (value === (base.fields[key] || "")) continue;
    if (value.trim()) (patch.fields ||= {})[key] = value.trim();
    else (patch.clear_fields ||= []).push(key);
  }
  return patch;
}
function dirty(id) { const draft = drafts.get(id); return Boolean(draft && (draft.upload || id === "new" || Object.keys(patchFor(draft)).length > 1)); }
function dirtyIds() { return [...drafts.keys()].filter(dirty); }

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    const state = await api("/api/state");
    const previous = JSON.stringify([tasks, fields, [...changed]]);
    tasks = state.tasks;
    fields = state.fields.fields;
    mediaLimits = state.media_limits || mediaLimits;
    changed = new Set(state.changed);
    needsRender ||= previous !== JSON.stringify([tasks, fields, [...changed]]);
    const folderParts = state.store.split(/[\\/]/).filter(Boolean);
    $("#project-name").textContent = folderParts.at(-1) === ".hippotask" ? folderParts.at(-2) || "Workspace" : folderParts.at(-1);
    $("#store-path").textContent = state.store;
    $("#store-path").title = state.store;
    for (const [id, draft] of drafts) {
      const latest = taskById(id);
      if (latest && latest.seq !== draft.base.seq && !draft.saving) {
        if (!dirty(id) && id !== active && id !== inline) drafts.set(id, newDraft(latest));
        else draft.latest = latest;
      }
    }
    connected = true;
    $("#connection").textContent = "Local session";
    $("#connection").className = "connection online";
    $("#connection-error").hidden = true;
    renderFilters();
    // Stable data must not steal keyboard focus every five seconds. A native
    // status menu, inline edit, or drag also owns its DOM until it finishes.
    const choosingState = document.activeElement?.matches("#tasks select");
    if (needsRender && !inline && !dragging && !choosingState) { renderTasks(); needsRender = false; }
    renderSelection();
  } catch (error) {
    connected = false;
    $("#connection").textContent = "Disconnected";
    $("#connection").className = "connection offline";
    $("#connection-error").textContent = error.message;
    $("#connection-error").hidden = false;
    renderSelection();
  } finally { refreshing = false; }
}

function renderFilters() {
  const labelSelect = $("#label-filter"), label = labelSelect.value;
  const allLabels = [...new Set(tasks.flatMap((task) => task.labels))].sort();
  if (label && !allLabels.includes(label)) allLabels.push(label);
  const labelSignature = JSON.stringify(allLabels);
  if (labelSelect.dataset.signature !== labelSignature) {
    labelSelect.dataset.signature = labelSignature;
    labelSelect.innerHTML = options(Object.fromEntries(allLabels.map((label) => [label, label])), label, "All labels");
  }
  const signature = JSON.stringify(fields);
  if ($("#field-filters").dataset.signature !== signature) {
    $("#field-filters").dataset.signature = signature;
    $("#field-filters").innerHTML = fields.map((field) => {
      const values = field.values || [...new Set(tasks.map((t) => t.fields[field.field]).filter(Boolean))].sort();
      return `<select data-filter-field="${escapeHtml(field.field)}" aria-label="Filter by ${escapeHtml(field.display_name)}">${options(Object.fromEntries(values.map((v) => [v,v])), fieldFilters[field.field], `All ${field.display_name.toLowerCase()}`)}</select>`;
    }).join("");
  }
}

function visibleTasks() {
  const search = $("#search").value.trim().toLowerCase(), status = $("#status-filter").value;
  return tasks.filter((task) => {
    if (task.state === "cancelled" && !$("#show-cancelled").checked && status !== "cancelled") return false;
    if (status && (status === "blocked" ? !task.blocked : task.state !== status)) return false;
    if ($("#priority-filter").value && task.priority !== $("#priority-filter").value) return false;
    if ($("#label-filter").value && !task.labels.includes($("#label-filter").value)) return false;
    if ($("#export-filter").value && exportState(task) !== $("#export-filter").value) return false;
    if (Object.entries(fieldFilters).some(([key, value]) => value && task.fields[key] !== value)) return false;
    return !search || [task.title, task.body || "", task.num, ...task.labels, ...Object.values(task.fields)].join(" ").toLowerCase().includes(search);
  });
}

function badge(label, kind = "") { return `<span class="badge ${kind}">${escapeHtml(label)}</span>`; }
function taskBadges(task) {
  return (task.priority !== "none" ? badge(priorities[task.priority], task.priority) : "") +
    (task.blocked ? badge("Blocked", "blocked") : "") +
    (held(task) ? badge(`Held by ${task.lease.holder}`, "holder") : "") +
    (dirty(task.id) ? badge("Unsaved", "draft") : "") +
    task.labels.map((label) => badge(label)).join("") +
    fields.slice(0,2).filter((field) => task.fields[field.field]).map((field) => badge(task.fields[field.field])).join("");
}
function stateSelect(task, draft) {
  return `<select class="card-state" aria-label="Status for task ${task.num}" ${draft ? 'data-input="state"' : `data-move="${task.id}"`} ${held(task) ? "disabled title=\"This task is held by another worker\"" : ""}>${options(statuses, draft ? draft.values.state : task.state)}</select>`;
}

function renderTasks() {
  const focused = document.activeElement;
  const focusAttribute = ["data-open","data-inline","data-select","data-move"].find((attribute) => focused?.closest("#tasks") && focused.hasAttribute(attribute));
  const focusValue = focusAttribute ? focused.getAttribute(focusAttribute) : null;
  const visible = visibleTasks();
  $("#board-view").setAttribute("aria-pressed", String(view === "board"));
  $("#list-view").setAttribute("aria-pressed", String(view === "list"));
  if (!visible.length) {
    $("#tasks").innerHTML = `<div class="empty"><h2>${tasks.length ? "No tasks match these filters" : "Room for your next idea"}</h2><p>${tasks.length ? "Try another search or clear a filter." : "Create a task here, or let your agents add work from the CLI."}</p>${!tasks.length ? '<button data-action="new">+ Create a task</button>' : ''}</div>`;
    renderSelection(); return;
  }
  if (view === "board") {
    const columns = Object.entries(statuses).filter(([state]) => state !== "cancelled" || $("#show-cancelled").checked || $("#status-filter").value === "cancelled");
    $("#tasks").innerHTML = `<div class="board ${columns.length === 4 ? "four" : ""}">${columns.map(([state, label]) => {
      const cards = visible.filter((task) => task.state === state);
      return `<section class="column" data-drop="${state}" aria-label="${label}"><div class="column-heading"><span class="state-mark ${state}"></span>${label}<span class="count">${cards.length}</span></div>${cards.map((task) => `<article class="card ${active === task.id ? "active" : ""}" data-task="${task.id}" draggable="${!held(task) && !dirty(task.id)}"><div class="card-top"><input type="checkbox" data-select="${task.id}" aria-label="Select task ${task.num}" ${selected.has(task.id) ? "checked" : ""}><span class="task-num">HT-${String(task.num).padStart(3,"0")}</span></div><button class="card-title" data-open="${task.id}">${escapeHtml(task.title)}</button>${task.body ? `<p class="card-description">${escapeHtml(task.body)}</p>` : ""}<div class="card-meta">${taskBadges(task)}</div><div class="card-bottom">${stateSelect(task)}<span class="export-label">${escapeHtml(exportLabel(task))}</span></div></article>`).join("") || '<p class="empty-column">No tasks here yet</p>'}</section>`;
    }).join("")}</div>`;
  } else {
    $("#tasks").innerHTML = `<div class="table-wrap"><table><thead><tr><th aria-label="Selection"></th><th>Task</th><th>Status</th><th>Priority</th><th>Assignee</th><th>Labels</th>${fields.map((field) => `<th>${escapeHtml(field.display_name)}</th>`).join("")}<th>Export</th><th>Actions</th></tr></thead><tbody>${visible.map(renderRow).join("")}</tbody></table></div>`;
    document.querySelectorAll(".edit-row").forEach((row) => {
      if (drafts.get(row.dataset.task)?.saving) row.querySelectorAll("input,select,button").forEach((input) => { input.disabled = true; });
    });
  }
  if (focusAttribute) $("#tasks").querySelector(`[${focusAttribute}="${CSS.escape(focusValue)}"]`)?.focus({preventScroll:true});
  renderSelection();
}

function fieldControl(field, value, extra = "") {
  const attr = `data-input-field="${escapeHtml(field.field)}" aria-label="${escapeHtml(field.display_name)}" ${extra}`;
  if (field.values) {
    const values = [...field.values];
    if (value && !values.includes(value)) values.push(value);
    return `<select ${attr}>${options(Object.fromEntries(values.map((v) => [v,v])), value, "Not set")}</select>`;
  }
  return `<input ${attr} value="${escapeHtml(value)}" placeholder="Not set">`;
}

function renderRow(task) {
  const d = inline === task.id ? draftFor(task.id) : null;
  const value = (key) => escapeHtml(d.values[key]);
  return `<tr data-task="${task.id}" class="${d ? "edit-row" : ""}"><td><input type="checkbox" data-select="${task.id}" aria-label="Select task ${task.num}" ${selected.has(task.id) ? "checked" : ""}></td><td class="title-cell"><span class="task-num">HT-${String(task.num).padStart(3,"0")}</span>${d ? `<input class="title-input" data-input="title" aria-label="Task title" value="${value("title")}">` : `<button class="card-title row-title" data-open="${task.id}">${escapeHtml(task.title)}</button>`}<div class="row-sub">${task.blocked ? badge("Blocked","blocked") : ""}${held(task) ? badge(task.lease.holder,"holder") : ""}${dirty(task.id) ? badge("Unsaved","draft") : ""}</div></td><td>${stateSelect(task,d)}</td><td>${d ? `<select data-input="priority" aria-label="Priority">${options(priorities,d.values.priority)}</select>` : badge(priorities[task.priority],task.priority)}</td><td>${d ? `<input data-input="assignee" aria-label="Assignee" value="${value("assignee")}">` : escapeHtml(task.assignee || "—")}</td><td>${d ? `<input data-input="labels" aria-label="Labels, comma separated" value="${value("labels")}">` : task.labels.map((tag) => badge(tag)).join(" ") || "—"}</td>${fields.map((field) => `<td>${d ? fieldControl(field,d.values.fields[field.field] || "") : escapeHtml(task.fields[field.field] || "—")}</td>`).join("")}<td>${badge(exportLabel(task),exportState(task))}</td><td><div class="row-actions">${d ? `<button data-save="${task.id}" class="primary" ${d.saving ? "disabled" : ""}>${d.saving ? "Saving…" : "Save"}</button><button data-discard="${task.id}" ${d.saving ? "disabled" : ""}>Cancel</button>` : `<button data-inline="${task.id}">Edit</button>`}</div></td></tr>${d?.error ? `<tr class="row-error"><td colspan="${8 + fields.length}"><p role="alert">${escapeHtml(d.error)}</p><button data-open="${task.id}">Open task to compare</button></td></tr>` : ""}`;
}

function renderSelection() {
  const visible = visibleTasks(), count = visible.filter((task) => selected.has(task.id)).length;
  const hidden = selected.size - count;
  $("#select-visible").checked = visible.length > 0 && count === visible.length;
  $("#select-visible").indeterminate = count > 0 && count < visible.length;
  $("#select-visible").disabled = !visible.length;
  $("#selection-label").textContent = selected.size ? `${selected.size} selected${hidden ? ` · ${hidden} hidden by filters` : ""}` : "Select visible";
  $("#clear-selection").hidden = !selected.size;
  $("#task-count").textContent = `${visible.length} of ${tasks.length} tasks`;
  $("#export-count").textContent = selected.size;
  $("#export-button").disabled = !selected.size || !connected;
  const countDrafts = dirtyIds().length;
  $("#draft-count").textContent = countDrafts ? `${countDrafts} unsaved ${countDrafts === 1 ? "draft" : "drafts"}` : "";
  const saveLabel = $(".save-label");
  if (saveLabel && active) saveLabel.textContent = dirty(active) ? "Unsaved changes" : "Up to date";
  for (const button of document.querySelectorAll("[data-save]")) button.disabled = Boolean(drafts.get(button.dataset.save)?.saving || drafts.get(button.dataset.save)?.upload);
}

function renderEditor() {
  clearDescriptionPreview();
  $("#editor").hidden = !active;
  $("#workspace").classList.toggle("has-editor",Boolean(active));
  if (!active) { $("#editor").innerHTML = ""; return; }
  const d = drafts.get(active), task = taskById(active) || d.base;
  const value = (key) => escapeHtml(d.values[key]);
  $("#editor").innerHTML = `<div class="editor-heading"><h2>${active === "new" ? "New task" : `Task ${task.num}`}</h2><button data-action="close-editor" aria-label="Close task editor">Close</button></div><div class="editor-body">${d.error ? `<div class="notice error" role="alert">${escapeHtml(d.error)}</div>` : ""}${d.latest && dirty(active) ? `<div class="notice">This task has a newer saved version. Your draft is preserved.<details open><summary>Latest saved values</summary><dl class="comparison"><dt>Title</dt><dd>${escapeHtml(d.latest.title)}</dd><dt>Status / Priority</dt><dd>${statuses[d.latest.state]} / ${priorities[d.latest.priority]}</dd><dt>Assignee / Labels</dt><dd>${escapeHtml(d.latest.assignee || "Unassigned")} / ${escapeHtml(d.latest.labels.join(", ") || "None")}</dd>${fields.map((f) => `<dt>${escapeHtml(f.display_name)}</dt><dd>${escapeHtml(d.latest.fields[f.field] || "Not set")}</dd>`).join("")}<dt>Description</dt><dd>${escapeHtml(d.latest.body || "No description")}</dd></dl></details><button data-action="rebase" ${d.upload ? "disabled" : ""}>Keep my changes on latest</button></div>` : ""}<label class="form-field"><span>Title</span><input data-input="title" id="editor-title" value="${value("title")}" placeholder="What needs doing?" ${d.saving ? "disabled" : ""}></label><div class="form-pair"><label class="form-field"><span>Status</span><select data-input="state" ${held(task) || active === "new" || d.saving ? "disabled" : ""}>${options(statuses,d.values.state)}</select></label><label class="form-field"><span>Priority</span><select data-input="priority" ${d.saving ? "disabled" : ""}>${options(priorities,d.values.priority)}</select></label></div>${held(task) ? `<p class="help">Held by ${escapeHtml(task.lease.holder)}. You can edit details; status belongs to the holder.</p>` : ""}<label class="form-field"><span>Assignee</span><input data-input="assignee" value="${value("assignee")}" placeholder="Unassigned" ${d.saving ? "disabled" : ""}></label><label class="form-field"><span>Labels · separated by commas</span><input data-input="labels" value="${value("labels")}" placeholder="e.g. design, backend" ${d.saving ? "disabled" : ""}></label>${fields.map((field) => `<label class="form-field"><span>${escapeHtml(field.display_name)}</span>${fieldControl(field,d.values.fields[field.field] || "",d.saving ? "disabled" : "")}</label>`).join("")}${descriptionEditor(d)}${active !== "new" ? `<details><summary>Dependencies & activity</summary><div id="task-history">Loading activity…</div></details>` : ""}</div><div class="editor-actions"><span class="save-label">${dirty(active) ? "Unsaved changes" : "Up to date"}</span><button data-discard="${active}" ${d.saving ? "disabled" : ""}>Discard</button><button data-save="${active}" class="primary" ${d.saving ? "disabled" : ""}>${d.saving ? "Saving…" : "Save"}</button></div>`;
  if (active !== "new") loadHistory(active);
  if (d.descriptionMode === "preview") previewDescription();
  renderUploadState(active);
}

function descriptionEditor(d) {
  const preview = d.descriptionMode === "preview";
  return `<section class="description-field form-field"><div class="description-heading"><label for="description-source">Description · Markdown</label><div class="view-switch description-switch" role="group" aria-label="Description view"><button type="button" data-description-mode="write" aria-pressed="${!preview}" aria-controls="description-source">Write</button><button type="button" data-description-mode="preview" aria-pressed="${preview}" aria-controls="description-preview">Preview</button></div></div><textarea id="description-source" data-input="body" spellcheck="true" placeholder="Add context for the next person…" ${d.saving ? "disabled" : ""} ${preview ? "hidden" : ""}>${escapeHtml(d.values.body)}</textarea><div id="description-preview" class="markdown-preview" role="region" aria-label="Description preview" aria-live="polite" tabindex="0" ${preview ? "" : "hidden"}></div><div class="media-tools"><button type="button" id="add-media" data-action="add-media" aria-describedby="media-formats media-storage">+ Add media</button><input type="file" id="media-picker" aria-label="Choose images or videos" accept=".png,.jpg,.jpeg,.gif,.webp,.mp4,.mov,.webm,image/png,image/jpeg,image/gif,image/webp,video/mp4,video/quicktime,video/webm" multiple hidden><span id="media-status" class="help" role="status" hidden></span></div><p id="media-errors" class="media-error" role="alert" hidden></p><p id="media-formats" class="help">${escapeHtml(mediaHelp())}</p><p id="media-storage" class="help">Files are copied to this local store when added, even if you discard the draft. Save attaches the description to this task.</p></section>`;
}

function clearDescriptionPreview() {
  descriptionRequest?.abort();
  descriptionRequest = null;
  for (const url of descriptionMediaUrls) URL.revokeObjectURL(url);
  descriptionMediaUrls = [];
}

function mediaHelp() {
  const mib = (bytes) => `${Math.round(bytes / 1024 / 1024 * 10) / 10} MiB`;
  return `PNG, JPEG, GIF, WebP · ${mib(mediaLimits.max_image_bytes)} per image. MP4, MOV, WebM · ${mib(mediaLimits.max_video_bytes)} per video.`;
}

function renderUploadState(id) {
  if (active !== id || !$("#add-media")) return;
  const d = drafts.get(id);
  $("#add-media").disabled = d.saving || Boolean(d.upload);
  $("#add-media").textContent = d.upload ? "Adding media…" : "+ Add media";
  $("#description-source").disabled = d.saving || Boolean(d.upload);
  $("#media-status").textContent = d.mediaMessage;
  $("#media-status").hidden = !d.mediaMessage;
  $("#media-errors").textContent = d.mediaErrors.join("\n");
  $("#media-errors").hidden = !d.mediaErrors.length;
  $("#media-formats").textContent = mediaHelp();
  renderSelection();
}

function chooseMedia() {
  const d = drafts.get(active);
  if (!d || d.saving || d.upload) return;
  // Capture the draft and insertion point before the native picker opens.
  // Its eventual result must not belong to whichever task is active later.
  const source = $("#description-source");
  mediaPicker = {id:active,d,offset:d.descriptionMode === "write" ? source.selectionEnd : d.values.body.length};
  $("#media-picker").value = "";
  $("#media-picker").click();
}

async function addMedia(files, selection) {
  if (!files.length || !selection || drafts.get(selection.id) !== selection.d) return;
  const {id,d} = selection;
  if (d.saving || d.upload) return;
  const controller = new AbortController();
  d.upload = controller; d.mediaErrors = [];
  let offset = Math.min(selection.offset,d.values.body.length), added = 0;
  const current = () => drafts.get(id) === d && d.upload === controller && !controller.signal.aborted;
  try {
    for (let index = 0; index < files.length; index++) {
      if (!current()) return;
      const file = files[index];
      d.mediaMessage = `Adding ${index + 1} of ${files.length}: ${file.name}`;
      renderUploadState(id);
      try {
        const video = /\.(mp4|mov|webm)$/i.test(file.name), image = /\.(png|jpe?g|gif|webp)$/i.test(file.name);
        const cap = video ? mediaLimits.max_video_bytes : image ? mediaLimits.max_image_bytes : Math.max(mediaLimits.max_image_bytes,mediaLimits.max_video_bytes);
        if (file.size > cap) throw new Error(`File exceeds the ${Math.round(cap / 1024 / 1024 * 10) / 10} MiB limit.`);
        const response = await fetch("/api/media",{method:"POST",headers:{"X-Hippo-Token":token,"Content-Type":"application/octet-stream"},body:file,signal:controller.signal});
        const result = await response.json();
        if (!current()) return;
        showWarnings(result.warnings || []);
        if (!response.ok) throw new Error(result.message || "Upload failed. Choose the file again.");
        const before = d.values.body.slice(0,offset), after = d.values.body.slice(offset);
        const link = `![${result.mime.startsWith("video/") ? "Video" : "Image"}](${result.path})`;
        const insert = `${before && !before.endsWith("\n\n") ? before.endsWith("\n") ? "\n" : "\n\n" : ""}${link}\n\n`;
        d.values.body = before + insert + after;
        offset = before.length + insert.length;
        added++;
        if (active === id && $("#description-source")) {
          const source = $("#description-source");
          source.value = d.values.body;
          source.setSelectionRange(offset,offset);
          if (d.descriptionMode === "preview") { clearDescriptionPreview(); previewDescription(); }
        }
      } catch (error) {
        if (!current()) return;
        d.mediaErrors.push(`${file.name}: ${error.message}`);
      }
    }
  } finally {
    if (current()) {
      d.upload = null;
      d.mediaMessage = added ? `Added ${added} ${added === 1 ? "file" : "files"} to your draft. Save to attach ${added === 1 ? "it" : "them"} to this task.` : "No files added. Your description is unchanged.";
      renderUploadState(id);
      renderTasks(); renderSelection();
      if (active !== id) toast(added ? `Media added to ${id === "new" ? "your new task draft" : `task ${d.base.num}'s draft`}. Open it to review and save.` : "Media upload failed. Open the task editor for details.",!added);
    }
  }
}

function setDescriptionMode(mode) {
  const d = drafts.get(active);
  if (!d || !["write","preview"].includes(mode) || d.descriptionMode === mode) return;
  d.descriptionMode = mode;
  clearDescriptionPreview();
  // Keep the textarea in place, including its selection and scroll position.
  $("#description-source").hidden = mode === "preview";
  $("#description-preview").hidden = mode !== "preview";
  for (const button of document.querySelectorAll("[data-description-mode]")) button.setAttribute("aria-pressed",String(button.dataset.descriptionMode === mode));
  if (mode === "preview") previewDescription();
}

async function previewDescription() {
  const panel = $("#description-preview"), source = drafts.get(active)?.values.body || "";
  const controller = new AbortController();
  descriptionRequest = controller;
  const current = () => descriptionRequest === controller && panel.isConnected;
  panel.classList.remove("preview-error");
  panel.setAttribute("aria-busy","true");
  panel.textContent = source.trim() ? "Rendering preview…" : "Nothing to preview yet.";
  try {
    if (!source.trim()) return;
    const result = await api("/api/markdown/preview","POST",{markdown:source},controller.signal);
    if (!current()) return;
    // Only the server's restricted Markdown renderer supplies this HTML.
    panel.innerHTML = result.html;
    const mediaCache = new Map();
    for (const placeholder of panel.querySelectorAll("[data-media]")) {
      if (!current()) return;
      try {
        const name = placeholder.dataset.media;
        if (!mediaCache.has(name)) {
          const response = await fetch(`/api/${name}`,{headers:{"X-Hippo-Token":token},signal:controller.signal});
          if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || "Media unavailable");
          }
          const blob = await response.blob();
          if (!current()) return;
          const url = URL.createObjectURL(blob);
          descriptionMediaUrls.push(url);
          mediaCache.set(name,{url,type:blob.type});
        }
        const {url,type} = mediaCache.get(name), video = type.startsWith("video/");
        const element = document.createElement(video ? "video" : "img");
        const caption = placeholder.dataset.caption;
        if (video) { element.controls = true; element.preload = "metadata"; element.setAttribute("aria-label",caption || "Description video"); }
        else element.alt = caption;
        const unavailable = () => {
          if (!current()) return;
          placeholder.replaceChildren(document.createTextNode(caption ? `${caption} — ` : ""),document.createTextNode("This media could not be displayed."));
          placeholder.classList.add("unavailable");
        };
        element.addEventListener("error",unavailable,{once:true});
        element.src = url;
        placeholder.replaceChildren(element);
        if (caption) { const label = document.createElement("small"); label.textContent = caption; placeholder.append(label); }
      } catch (error) {
        if (!current()) return;
        placeholder.classList.add("unavailable");
        placeholder.textContent = `${placeholder.dataset.caption || "Media"} — ${error.message}`;
      }
    }
  } catch (error) {
    if (!current()) return;
    panel.classList.add("preview-error");
    panel.textContent = `Preview unavailable. ${error.message} Switch to Write to keep editing.`;
  } finally {
    if (current()) panel.setAttribute("aria-busy","false");
  }
}

async function loadHistory(id) {
  try {
    const detail = await api(`/api/task/${id}`);
    if (active !== id || !$("#task-history")) return;
    const blockers = detail.relations.filter((r) => r.rel === "blocked-by").map((r) => taskById(r.task)).filter(Boolean);
    const historyRows = [...detail.events].reverse().slice(0,30);
    $("#task-history").innerHTML = `${blockers.length ? `<p>Blocked by: ${blockers.map((t) => `#${t.num} ${escapeHtml(t.title)} (${statuses[t.state]})`).join("; ")}</p>` : "<p>No dependencies.</p>"}<ol>${historyRows.map((event) => `<li>${escapeHtml(event.actor)} · ${escapeHtml(event.type)}${event.applied ? "" : " (no change)"}<br>${escapeHtml(new Date(event.ts).toLocaleString())}<div class="history-value">${escapeHtml(Object.values(event.data || {}).map((v) => typeof v === "object" ? JSON.stringify(v) : v).join(" · "))}</div></li>`).join("")}</ol>`;
  } catch (error) { if (active === id && $("#task-history")) $("#task-history").textContent = error.message; }
}

function openTask(id) {
  if (id !== "new" && !taskById(id)) return;
  active = id;
  inline = null;
  if (id !== "new") {
    if (!dirty(id)) drafts.set(id,newDraft(taskById(id)));
    else draftFor(id).latest = taskById(id).seq !== draftFor(id).base.seq ? taskById(id) : null;
  }
  renderTasks(); renderEditor();
}
function createTask() {
  if (!drafts.has("new")) drafts.set("new",newDraft({id:"new",seq:0,title:"",body:null,state:"todo",priority:"none",assignee:null,labels:[],fields:{},relations:[]}));
  openTask("new"); $("#editor-title").focus();
}
function discard(id) {
  if (drafts.get(id)?.saving) return;
  drafts.get(id)?.upload?.abort();
  drafts.delete(id);
  if (inline === id) inline = null;
  if (active === id) active = null;
  renderTasks(); renderEditor();
}

async function saveDraft(id) {
  const d = drafts.get(id);
  if (!d || d.saving || d.upload) return;
  if (id !== "new" && !dirty(id)) { toast("No changes to save."); return; }
  d.saving = true; d.error = "";
  if (active === id) renderEditor();
  if (inline === id) renderTasks();
  try {
    const payload = id === "new" ? {title:d.values.title,priority:d.values.priority,body:d.values.body.trim() ? d.values.body : null,assignee:d.values.assignee.trim() || null,labels:tags(d.values.labels),fields:Object.fromEntries(Object.entries(d.values.fields).filter(([,v]) => v.trim()))} : patchFor(d);
    const saved = await api(id === "new" ? "/api/tasks" : `/api/task/${id}`,id === "new" ? "POST" : "PATCH",payload);
    const index = tasks.findIndex((task) => task.id === saved.id);
    if (index < 0) tasks.push(saved); else tasks[index] = saved;
    drafts.delete(id);
    if (active === id) { active = saved.id; drafts.set(saved.id,newDraft(saved)); }
    if (inline === id) inline = null;
    toast(id === "new" ? "Task created." : "Changes saved to HippoTask.");
    await refresh(); renderTasks(); renderEditor();
  } catch (error) {
    d.error = error.message;
    if (error.kind === "stale") {
      try { d.latest = await api(`/api/task/${id}`); }
      catch (latestError) { d.error += `\n${latestError.message}`; }
    }
    if (error.kind === "connection") d.error += "\nThe save was not confirmed. Check the task list before retrying; it may already have succeeded.";
    toast(error.kind === "connection" ? "Save not confirmed. Your draft is still here." : "Nothing saved. Your draft is still here.",true);
  } finally {
    d.saving = false;
    if (active === id) renderEditor();
    if (inline === id) renderTasks();
    renderSelection();
  }
}

function rebaseDraft() {
  const d = drafts.get(active);
  if (!d?.latest || d.saving || d.upload) return;
  const patch = patchFor(d), next = newDraft(d.latest);
  for (const key of ["title","body","state","priority","assignee"]) if (Object.hasOwn(patch,key)) next.values[key] = patch[key];
  if (patch.unassign) next.values.assignee = "";
  if (patch.label_add || patch.label_remove) next.values.labels = [...new Set([...next.base.labels.filter((tag) => !(patch.label_remove || []).includes(tag)),...(patch.label_add || [])])].join(", ");
  Object.assign(next.values.fields,patch.fields || {});
  for (const key of patch.clear_fields || []) next.values.fields[key] = "";
  drafts.set(active,next); renderEditor(); renderSelection();
  toast("Draft updated against the latest task. Review it, then save.");
}

async function moveTask(id, state) {
  const task = taskById(id);
  if (!task || task.state === state) return;
  if (dirty(id) || held(task)) { toast(dirty(id) ? "Save or discard this task's draft before moving it." : "The holder controls this task's status.",true); renderTasks(); return; }
  try {
    const saved = await api(`/api/task/${id}`,"PATCH",{base:task.seq,state});
    tasks[tasks.findIndex((t) => t.id === id)] = saved;
    if (active === id) drafts.set(id,newDraft(saved));
    renderTasks(); renderEditor(); toast(`Moved to ${statuses[state]}.`); await refresh();
  } catch (error) { toast(error.message,true); await refresh(); renderTasks(); }
}

// CSV parser handles quoted commas, quotes, and multiline descriptions. The
// preview comes from the same renderer used to write the actual file.
function parseCsv(csv) {
  const rows = []; let row = [], cell = "", quoted = false;
  for (let i = 0; i < csv.length; i++) {
    const c = csv[i];
    if (c === '"') {
      if (quoted && csv[i+1] === '"') { cell += '"'; i++; }
      else quoted = !quoted;
    } else if (c === "," && !quoted) { row.push(cell); cell = ""; }
    else if (c === "\n" && !quoted) { row.push(cell); rows.push(row); row = []; cell = ""; }
    else if (c !== "\r" || quoted) cell += c;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}

function openExport() {
  const unsaved = dirtyIds();
  if (unsaved.length) { openTask(unsaved[0]); toast("Save or discard your drafts before exporting.",true); return; }
  exportIds = [...selected]; previewData = null;
  $("#include-again").checked = false;
  $("#export-path").value = "";
  $("#export-result").hidden = true;
  $("#export-dialog").showModal();
  previewExport();
}
async function previewExport() {
  const version = ++previewVersion;
  const again = $("#include-again").checked;
  previewData = null;
  $("#save-export").disabled = true;
  $("#export-error").hidden = true;
  $("#export-result").hidden = true;
  $("#duplicate-hint").hidden = !again;
  $("#export-summary").textContent = "Preparing preview…";
  $("#export-preview").textContent = "";
  try {
    const result = await api("/api/export/preview","POST",{ids:exportIds,again});
    if (version !== previewVersion) return;
    previewData = {...result,ids:[...exportIds],again};
    const skipped = exportIds.length - result.exported.length;
    $("#export-summary").textContent = `${result.exported.length} ${result.exported.length === 1 ? "task" : "tasks"} to export · ${skipped} skipped${result.changed.length ? ` · ${result.changed.length} changed since export` : ""}${skipped ? ". Previously exported tasks are skipped unless included again." : ""}`;
    const [head,...rows] = parseCsv(result.csv);
    $("#export-preview").innerHTML = rows.length ? `<table><thead><tr>${head.map((cell) => `<th>${escapeHtml(cell)}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table>` : '<div class="empty"><h2>No new tasks in this selection</h2><p>These tasks have already been exported. Include them again only if you want new rows.</p></div>';
    if (!$("#export-path").value) $("#export-path").value = result.suggested_path;
    $("#save-export").disabled = !result.exported.length || exporting;
  } catch (error) {
    if (version !== previewVersion) return;
    $("#export-summary").textContent = "Preview unavailable";
    $("#export-error").textContent = error.message;
    $("#export-error").hidden = false;
  }
}
async function saveExport() {
  if (!previewData || exporting) return;
  exporting = true;
  const preview = previewData;
  $("#save-export").disabled = true;
  $("#include-again").disabled = true;
  $("#refresh-preview").disabled = true;
  $("#save-export").textContent = "Saving…";
  try {
    const result = await api("/api/export/save","POST",{ids:preview.ids,again:preview.again,review:preview.review,path:$("#export-path").value});
    for (const task of result.exported) selected.delete(task.id);
    $("#export-result").textContent = `Saved ${result.exported.length} ${result.exported.length === 1 ? "task" : "tasks"} to:\n${result.file}${result.media_files.length ? `\n${result.media_files.length} media files copied beside the CSV.` : ""}\nImport this file into Notion. Exporting does not confirm an import.`;
    $("#export-result").hidden = false;
    $("#export-error").hidden = true;
    previewData = null;
    await refresh();
  } catch (error) {
    $("#export-error").textContent = error.message;
    $("#export-error").hidden = false;
    if (error.kind === "stale") previewData = null;
  } finally {
    exporting = false;
    $("#save-export").disabled = !previewData;
    $("#save-export").textContent = "Save CSV";
    $("#include-again").disabled = false;
    $("#refresh-preview").disabled = false;
  }
}

document.addEventListener("click", (event) => {
  const button = event.target.closest("button");
  if (!button || button.disabled) return;
  if (button.dataset.open) openTask(button.dataset.open);
  if (button.dataset.inline) {
    const id = button.dataset.inline;
    if (!dirty(id)) drafts.set(id,newDraft(taskById(id)));
    inline = id; if (active === id) active = null;
    renderEditor(); renderTasks();
    $(`tr[data-task="${id}"] .title-input`)?.focus();
  }
  if (button.dataset.save) saveDraft(button.dataset.save);
  if (button.dataset.discard) discard(button.dataset.discard);
  if (button.dataset.descriptionMode) setDescriptionMode(button.dataset.descriptionMode);
  if (button.dataset.action === "add-media") chooseMedia();
  if (button.dataset.action === "new") createTask();
  if (button.dataset.action === "close-editor") { active = null; renderEditor(); renderTasks(); }
  if (button.dataset.action === "rebase") rebaseDraft();
});
function inputDraft(event) {
  const input = event.target;
  if (!input.hasAttribute("data-input") && !input.hasAttribute("data-input-field")) return;
  const id = input.closest("#editor") ? active : input.closest("tr[data-task]")?.dataset.task;
  const d = drafts.get(id);
  if (!d || d.saving) return;
  if (input.hasAttribute("data-input-field")) d.values.fields[input.dataset.inputField] = input.value;
  else d.values[input.dataset.input] = input.value;
  renderSelection();
}
document.addEventListener("input",inputDraft);
document.addEventListener("change", (event) => {
  const input = event.target;
  if (input.id === "media-picker") { const selection = mediaPicker; mediaPicker = null; addMedia([...input.files],selection); return; }
  inputDraft(event);
  if (input.dataset.select) { if (input.checked) selected.add(input.dataset.select); else selected.delete(input.dataset.select); renderSelection(); }
  if (input.dataset.move) moveTask(input.dataset.move,input.value);
  if (input.hasAttribute("data-filter-field")) { fieldFilters[input.dataset.filterField] = input.value; renderTasks(); }
});
document.addEventListener("keydown", (event) => {
  const row = event.target.closest("tr.edit-row");
  if (row && event.target.tagName === "INPUT" && event.target.type !== "checkbox") {
    if (event.key === "Enter") { event.preventDefault(); saveDraft(row.dataset.task); }
    if (event.key === "Escape") { event.preventDefault(); discard(row.dataset.task); }
  }
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && active) { event.preventDefault(); saveDraft(active); }
});
$("#tasks").addEventListener("dragstart", (event) => {
  const card = event.target.closest(".card");
  if (!card || card.draggable === false || event.target.closest("button,input,select")) { event.preventDefault(); return; }
  event.dataTransfer.setData("text/plain",card.dataset.task);
  event.dataTransfer.effectAllowed = "move";
  dragging = true;
});
$("#tasks").addEventListener("dragover", (event) => {
  const column = event.target.closest("[data-drop]");
  if (column) { event.preventDefault(); event.dataTransfer.dropEffect = "move"; column.classList.add("drop-target"); }
});
$("#tasks").addEventListener("dragleave", (event) => {
  const column = event.target.closest("[data-drop]");
  if (column && !column.contains(event.relatedTarget)) column.classList.remove("drop-target");
});
$("#tasks").addEventListener("drop", (event) => {
  event.preventDefault();
  document.querySelectorAll(".drop-target").forEach((el) => el.classList.remove("drop-target"));
  const column = event.target.closest("[data-drop]");
  if (column) moveTask(event.dataTransfer.getData("text/plain"),column.dataset.drop);
});
document.addEventListener("dragend", () => { dragging = false; document.querySelectorAll(".drop-target").forEach((el) => el.classList.remove("drop-target")); });
$("#board-view").onclick = () => { view = "board"; inline = null; renderTasks(); };
$("#list-view").onclick = () => { view = "list"; renderTasks(); };
$("#new-task").onclick = createTask;
$("#export-button").onclick = openExport;
$("#select-visible").onchange = (event) => { for (const task of visibleTasks()) { if (event.target.checked) selected.add(task.id); else selected.delete(task.id); } renderTasks(); };
$("#clear-selection").onclick = () => { selected.clear(); renderTasks(); };
$("#search").oninput = renderTasks;
for (const selector of ["#status-filter","#priority-filter","#label-filter","#export-filter","#show-cancelled"]) $(selector).onchange = renderTasks;
$("#close-export").onclick = () => { if (!exporting) { previewVersion++; $("#export-dialog").close(); } };
$("#export-dialog").addEventListener("cancel", (event) => { if (exporting) event.preventDefault(); else previewVersion++; });
$("#include-again").onchange = previewExport;
$("#refresh-preview").onclick = previewExport;
$("#save-export").onclick = saveExport;
window.addEventListener("beforeunload", (event) => { if (dirtyIds().length || exporting) { event.preventDefault(); event.returnValue = ""; } });
window.addEventListener("focus",refresh);
setInterval(() => { if (!document.hidden) refresh(); },5000);
refresh();
