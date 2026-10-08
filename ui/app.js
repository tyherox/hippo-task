"use strict";

// One collection, one selection, and one set of drafts power both views.
// Drafts stay in this tab; only an explicit Save reaches the local ledger.
const $ = (selector) => document.querySelector(selector);
const escapeHtml = (value) => String(value ?? "").replace(/[&<>"']/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const statuses = {todo:"To do", doing:"In progress", done:"Done", cancelled:"Cancelled"};
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
let tasks = [], fields = [], changed = new Set(), active = null, inline = null, view = "list";
let workspaceScope = "all", lastSelected = null;
let bulkBusy = false, bulkTargets = new Set(), bulkResult = null, bulkUndo = [];
let fieldFilters = {}, connected = false, refreshing = false, refreshAgain = false, writes = 0, dragging = false, needsRender = true, toastTimer;
let exportIds = [], previewData = null, previewVersion = 0, exporting = false;
let descriptionRequest = null, descriptionMediaUrls = [], descriptionTimer;
// The project's task conventions (ADR-012): a template for new descriptions,
// and fields every task should carry.
let taskFormat = {guide:null, template:null, required_fields:[], required_sections:[]};
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
// Every change goes through here, so a task list read while it was in flight
// can be recognized as possibly older than the change and read again.
async function write(path, method, body) {
  try { return await api(path, method, body); }
  finally { writes++; }
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
  return {base:structuredClone(task), values:{title:task.title, body:task.body || "", state:task.state, priority:task.priority, assignee:task.assignee || "", labels:task.labels.join(", "), fields:{...task.fields}}, error:"", latest:null, saving:false, descriptionMode:task.body?.trim() ? "preview" : "write", upload:null, mediaMessage:"", mediaErrors:[]};
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
  if (refreshing) { refreshAgain = true; return; }
  refreshing = true;
  try {
    let state;
    do {
      refreshAgain = false;
      const before = writes;
      state = await api("/api/state");
      if (writes !== before) refreshAgain = true;
    } while (refreshAgain);
    const previous = JSON.stringify([tasks, fields, [...changed]]);
    tasks = state.tasks;
    fields = state.fields.fields;
    mediaLimits = state.media_limits || mediaLimits;
    taskFormat = state.format || taskFormat;
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
    $("#connection").textContent = "On this computer";
    $("#connection").className = "connection online";
    $("#connection").title = "On this computer · changes save to the local task store";
    $("#connection-error").hidden = true;
    renderFilters();
    // Stable data must not steal keyboard focus every five seconds. A native
    // status menu, inline edit, or drag also owns its DOM until it finishes.
    const choosingState = document.activeElement?.matches("#tasks select");
    if (needsRender && !inline && !dragging && !choosingState && !bulkBusy) { renderTasks(); needsRender = false; }
    renderSelection();
  } catch (error) {
    connected = false;
    $("#connection").textContent = "Disconnected";
    $("#connection").className = "connection offline";
    $("#connection").title = error.message;
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
    labelSelect.innerHTML = options(Object.fromEntries(allLabels.map((label) => [label, label])), label, "All tags");
  }
  const signature = JSON.stringify(fields);
  if ($("#field-filters").dataset.signature !== signature) {
    $("#field-filters").dataset.signature = signature;
    $("#field-filters").innerHTML = fields.map((field) => {
      const values = field.values || [...new Set(tasks.map((t) => t.fields[field.field]).filter(Boolean))].sort();
      return `<select data-filter-field="${escapeHtml(field.field)}" aria-label="Filter by ${escapeHtml(field.display_name)}">${options(Object.fromEntries(values.map((v) => [v,v])), fieldFilters[field.field], `All ${field.display_name.toLowerCase()}`)}</select>`;
    }).join("");
    // Removing a value is its own menu choice, so a blank or untouched value
    // control can never clear anything.
    const bulkField = $("#bulk-field"), chosen = bulkField.value || "state";
    const set = {state:"Status",priority:"Priority",assignee:"Owner","add-tags":"Add tags","remove-tags":"Remove tags",...Object.fromEntries(fields.map(f => [`field:${f.field}`,f.display_name]))};
    const remove = {unassign:"Unassign owner",...Object.fromEntries(fields.map(f => [`clear:${f.field}`,`Clear ${f.display_name}`]))};
    bulkField.innerHTML = `<optgroup label="Set">${options(set,chosen)}</optgroup><optgroup label="Remove">${options(remove,chosen)}</optgroup>`;
    if (bulkField.value !== chosen) renderBulkValue();
  }
}

function visibleTasks() {
  const search = $("#search").value.trim().toLowerCase(), status = $("#status-filter").value;
  return tasks.filter((task) => {
    if (workspaceScope === "active" && !["todo","doing"].includes(task.state)) return false;
    if (workspaceScope === "done" && task.state !== "done") return false;
    if (workspaceScope === "blocked" && !task.blocked) return false;
    if (workspaceScope === "drafts" && !dirty(task.id)) return false;
    if (task.state === "cancelled" && !$("#show-cancelled").checked && status !== "cancelled") return false;
    if (status && (status === "blocked" ? !task.blocked : task.state !== status)) return false;
    if ($("#priority-filter").value && task.priority !== $("#priority-filter").value) return false;
    if ($("#label-filter").value && !task.labels.includes($("#label-filter").value)) return false;
    if ($("#export-filter").value && exportState(task) !== $("#export-filter").value) return false;
    if (Object.entries(fieldFilters).some(([key, value]) => value && task.fields[key] !== value)) return false;
    return !search || [task.title, task.body || "", task.num, task.assignee || "", ...task.labels, ...Object.values(task.fields)].join(" ").toLowerCase().includes(search);
  }).sort((a,b) => {
    const order = $("#sort-order").value;
    if (order === "priority") { const rank={urgent:0,high:1,med:2,low:3,none:4}; return rank[a.priority]-rank[b.priority] || a.num-b.num; }
    if (order === "newest") return b.num-a.num;
    if (order === "title") return a.title.localeCompare(b.title) || a.num-b.num;
    return a.num-b.num;
  });
}

function reviewTasks() {
  const visible = visibleTasks();
  return view === "board" || $("#group-by").value === "state" ? Object.keys(statuses).flatMap(state => visible.filter(task => task.state === state)) : visible;
}

function selectTask(id, checked, range = false) {
  if (bulkBusy) return;
  const order = reviewTasks().map(t => t.id), start = order.indexOf(lastSelected), end = order.indexOf(id);
  const ids = range && start >= 0 && end >= 0 ? order.slice(Math.min(start,end),Math.max(start,end)+1) : [id];
  for (const taskId of ids) { if (checked) selected.add(taskId); else selected.delete(taskId); }
  lastSelected = id;
  renderSelection();
}

function bulkPatch(task, change) {
  const patch = {}, value = String(change.value ?? "").trim();
  if (change.field === "state" || change.field === "priority") {
    const allowed = change.field === "state" ? statuses : priorities;
    if (!Object.hasOwn(allowed,value)) throw new Error(`Choose a ${change.field === "state" ? "status" : "priority"} first.`);
    if (task[change.field] !== value) patch[change.field] = value;
  } else if (change.field === "assignee") {
    if (!value) throw new Error("Enter an owner, or choose Unassign owner.");
    if ((task.assignee || "") !== value) patch.assignee=value;
  } else if (change.field === "unassign") {
    if (task.assignee) patch.unassign=true;
  } else if (["add-tags","remove-tags"].includes(change.field)) {
    const values = tags(value);
    if (!values.length) throw new Error("Enter at least one tag.");
    const removing = change.field === "remove-tags";
    const labels = values.filter(label => removing ? task.labels.includes(label) : !task.labels.includes(label));
    if (labels.length) patch[removing ? "label_remove" : "label_add"] = labels;
  } else if (change.field.startsWith("field:")) {
    const name = change.field.slice(6), field = fields.find(f => f.field === name);
    if (!field) throw new Error("Choose a property to change.");
    if (!value) throw new Error(`Choose a value for ${field.display_name}, or choose Clear ${field.display_name}.`);
    if (field.values && !field.values.includes(value)) throw new Error("Choose a valid value for this field.");
    if ((task.fields[name] || "") !== value) patch.fields={[name]:value};
  } else if (change.field.startsWith("clear:")) {
    const name = change.field.slice(6);
    if (!fields.some(f => f.field === name)) throw new Error("Choose a property to change.");
    if (task.fields[name]) patch.clear_fields=[name];
  } else throw new Error("Choose a property to change.");
  return patch;
}

function bulkChoice() { return {field:$("#bulk-field").value, value:$("#bulk-value")?.value ?? ""}; }
function bulkReady(change) {
  if (change.field === "unassign" || change.field.startsWith("clear:")) return true;
  if (["add-tags","remove-tags"].includes(change.field)) return tags(String(change.value ?? "")).length > 0;
  return Boolean(String(change.value ?? "").trim());
}
function bulkNote() {
  if ($("#bulk-field").value !== "state") return "";
  const count = [...selected].map(taskById).filter(task => task && held(task)).length;
  return count ? `${count} held by ${count === 1 ? "another worker keeps its" : "other workers keep their"} status` : "";
}

function reversePatch(task, patch) {
  const inverse = {};
  for (const key of ["state","priority"]) if (Object.hasOwn(patch,key)) inverse[key]=task[key];
  if (patch.assignee || patch.unassign) { if (task.assignee) inverse.assignee=task.assignee; else inverse.unassign=true; }
  if (patch.label_add) inverse.label_remove=patch.label_add;
  if (patch.label_remove) inverse.label_add=patch.label_remove;
  for (const name of [...Object.keys(patch.fields || {}),...(patch.clear_fields || [])]) {
    if (task.fields[name]) (inverse.fields ||= {})[name]=task.fields[name];
    else (inverse.clear_fields ||= []).push(name);
  }
  return inverse;
}

async function runBulkChange(change) {
  if (bulkBusy || !selected.size) return;
  let plan;
  try {
    plan = [...selected].map(id => {
      const task = taskById(id);
      if (!task) return {id,num:"?",missing:true,patch:{}};
      const patch = bulkPatch(task,change);
      return {id,num:task.num,base:task.seq,patch,inverse:reversePatch(task,patch)};
    });
  } catch (error) { toast(error.message,true); return; }
  return executeBulk(plan,false);
}

async function undoBulkChange() {
  if (bulkBusy || !bulkUndo.length) return;
  const plan = bulkUndo.map(item => ({...item}));
  return executeBulk(plan,true);
}

// An undo can't be retried (it is used up either way), so its failures
// explain what was left alone rather than suggesting another attempt.
const bulkFailures = {
  disconnected:["Not attempted because the connection was lost.","Not attempted because the connection was lost."],
  missing:["This task is no longer available.","This task is no longer available."],
  draft:["Save or discard this task’s unsaved changes first.","It has unsaved changes, so undo left it as it is."],
  held:["Someone else is working on this task and controls its status.","Someone else is working on this task, so undo left its status as it is."],
  connection:["Save not confirmed. Check this task before retrying; it may have been updated.","Undo not confirmed. Check this task; it may have been changed back."],
  stale:["Changed since you reviewed it. Open the task, review it, then try again.","Changed after the batch, so undo left it as it is. Open the task to review it."],
};

async function executeBulk(plan, undo) {
  bulkBusy=true; bulkTargets=new Set(plan.map(item=>item.id));
  bulkResult=null;
  const result={updated:[],unchanged:[],failed:[],undo}, nextUndo=[];
  const failure=(kind) => bulkFailures[kind][undo ? 1 : 0];
  let disconnected=false;
  renderSelection(); renderBulkResult(); renderEditor();
  try {
    for (const item of plan) {
      $("#bulk-progress").textContent=`${undo ? "Undoing" : "Updating"} ${result.updated.length+result.unchanged.length+result.failed.length+1} of ${plan.length}…`;
      let reason="";
      if (disconnected) reason=failure("disconnected");
      else if (item.missing || !taskById(item.id)) reason=failure("missing");
      else if (dirty(item.id) || drafts.get(item.id)?.saving) reason=failure("draft");
      else if (item.patch.state && held(taskById(item.id))) reason=failure("held");
      if (reason) { result.failed.push({...item,message:reason}); continue; }
      if (!Object.keys(item.patch).length) { result.unchanged.push(item); continue; }
      try {
        const saved=await write(`/api/task/${item.id}`,"PATCH",{base:item.base,...item.patch});
        tasks[tasks.findIndex(t=>t.id===item.id)]=saved;
        if (drafts.has(item.id)) drafts.set(item.id,newDraft(saved));
        result.updated.push(item);
        if (!undo) nextUndo.push({id:item.id,num:item.num,base:saved.seq,patch:item.inverse});
      } catch (error) {
        disconnected=error.kind === "connection";
        result.failed.push({...item,message:disconnected ? failure("connection") : error.kind === "stale" ? failure("stale") : error.message});
      }
    }
  } finally {
    // Undo is for the most recent batch that changed something; running it
    // uses it up. Conditional bases keep an older undo from overwriting newer work.
    if (undo) bulkUndo=[];
    else if (nextUndo.length) bulkUndo=nextUndo;
    bulkBusy=false; bulkTargets.clear(); bulkResult=result;
    $("#bulk-progress").textContent="";
    await refresh(); renderTasks(); renderEditor(); renderBulkResult(); renderSelection();
  }
  return result;
}

function renderBulkResult() {
  const panel=$("#bulk-result"); panel.hidden=!bulkResult;
  if (!bulkResult) return;
  const {updated,unchanged,failed,undo}=bulkResult;
  panel.innerHTML=`<div class="bulk-result-heading"><span>${updated.length} ${undo ? "undone" : "updated"}${unchanged.length ? ` · ${unchanged.length} already matched` : ""}${failed.length ? ` · ${failed.length} need attention` : ""}</span><div>${bulkUndo.length ? `<button data-action="undo-bulk" class="quiet-button">${updated.length ? "Undo last batch" : "Undo previous batch"}</button>` : ""}<button data-action="dismiss-bulk" class="quiet-button" aria-label="Dismiss bulk edit result">×</button></div></div>${failed.length ? `<details open><summary>Review tasks that weren’t confirmed</summary><ul>${failed.map(item=>`<li><button data-open="${escapeHtml(item.id)}">Task ${item.num}</button><span>${escapeHtml(item.message)}</span></li>`).join("")}</ul></details>` : ""}`;
}

function renderBulkValue() {
  const field=$("#bulk-field").value, definition=fields.find(f=>`field:${f.field}`===field);
  const removing=fields.find(f=>`clear:${f.field}`===field);
  const name=definition?.display_name || {state:"Status",priority:"Priority",assignee:"Owner"}[field];
  const choices=field === "state" ? statuses : field === "priority" ? priorities : definition?.values ? Object.fromEntries(definition.values.map(value=>[value,value])) : null;
  if (field === "unassign" || removing) $("#bulk-value-control").innerHTML=`<span class="bulk-explain">${field === "unassign" ? "Removes the owner from the selected tasks" : `Removes ${escapeHtml(removing.display_name)} from the selected tasks`}</span>`;
  else if (choices) $("#bulk-value-control").innerHTML=`<select id="bulk-value" aria-label="New ${escapeHtml(name.toLowerCase())}">${options(choices,"",`Choose ${name.toLowerCase()}…`)}</select>`;
  else $("#bulk-value-control").innerHTML=`<input id="bulk-value" aria-label="${name ? `New ${escapeHtml(name.toLowerCase())}` : "Tags to change"}" placeholder="${field === "assignee" ? "Owner" : name ? escapeHtml(name) : "Tags, separated by commas"}">`;
  renderSelection();
}
function adjacentTask(direction) {
  const review = reviewTasks(), index = review.findIndex(task => task.id === active);
  return index < 0 ? null : review[index + direction] || null;
}
function navigateTask(direction) {
  const next = adjacentTask(direction);
  if (next) openTask(next.id);
}
async function saveAndNext() {
  const id = active, next = adjacentTask(1);
  if (!next || !id) return;
  // Saving may remove the current task from a status/search filter.
  const saved = await saveDraft(id);
  if (saved && active === id) openTask(next.id);
}
function renderTaskNavigation() {
  const navigation = $("#task-navigation");
  if (!active || !navigation) return;
  const review = reviewTasks(), index = review.findIndex(task => task.id === active);
  navigation.innerHTML = `<span class="review-position">${active === "new" ? "New" : index < 0 ? "Outside this view" : `${index + 1} / ${review.length}`}</span><label class="task-chooser"><span class="sr-only">Jump to task</span><select id="task-chooser" title="Jump to another task"><option value="">Jump to…</option>${review.map(task => `<option value="${escapeHtml(task.id)}" ${task.id === active ? "selected" : ""}>#${task.num} · ${escapeHtml(task.title)}</option>`).join("")}</select></label><button data-navigate="-1" aria-label="Previous task" title="Previous task · Alt + Left" ${index <= 0 ? "disabled" : ""}>↑</button><button data-navigate="1" aria-label="Next task" title="Next task · Alt + Right" ${index < 0 || index === review.length - 1 ? "disabled" : ""}>↓</button>`;
  const nextButton = $("#save-next");
  if (nextButton) nextButton.disabled = !adjacentTask(1) || bulkTargets.has(active) || Boolean(drafts.get(active)?.saving || drafts.get(active)?.upload);
}

function badge(label, kind = "") { return `<span class="badge ${kind}">${escapeHtml(label)}</span>`; }
function taskBadges(task) {
  return (task.priority !== "none" ? badge(priorities[task.priority], task.priority) : "") +
    (task.blocked ? badge("Waiting on tasks", "blocked") : "") +
    (held(task) ? badge(`Held by ${task.lease.holder}`, "holder") : "") +
    (dirty(task.id) ? badge("Unsaved", "draft") : "") +
    task.labels.map((label) => badge(label)).join("") +
    fields.slice(0,2).filter((field) => task.fields[field.field]).map((field) => badge(task.fields[field.field])).join("");
}
function stateSelect(task, draft) {
  return `<select class="card-state" aria-label="Status for task ${task.num}" ${draft ? 'data-input="state"' : `data-move="${task.id}"`} ${held(task) ? "disabled title=\"This task is held by another worker\"" : ""}>${options(statuses, draft ? draft.values.state : task.state)}</select>`;
}

function renderTasks() {
  const listScroll = $("#tasks").scrollTop;
  const pendingNew = workspaceScope === "drafts" && drafts.has("new") ? `<div class="new-draft-row"><span class="draft-dot"></span><button data-open="new">${escapeHtml(drafts.get("new").values.title || "Untitled task")}</button><span>New · not saved yet</span></div>` : "";
  const focused = document.activeElement;
  const focusAttribute = ["data-open","data-inline","data-select","data-move"].find((attribute) => focused?.closest("#tasks") && focused.hasAttribute(attribute));
  const focusValue = focusAttribute ? focused.getAttribute(focusAttribute) : null;
  const visible = visibleTasks();
  $("#board-view").setAttribute("aria-pressed", String(view === "board"));
  $("#list-view").setAttribute("aria-pressed", String(view === "list"));
  $("#group-by").disabled = view === "board";
  if (!visible.length) {
    $("#tasks").innerHTML = pendingNew || `<div class="empty"><h2>${tasks.length ? "No tasks match these filters" : "Room for your next idea"}</h2><p>${tasks.length ? "Try another search or clear a filter." : "Create a task to get started. You can add details and screenshots as you go."}</p><button data-action="${tasks.length ? "reset-filters" : "new"}">${tasks.length ? "Show all tasks" : "+ Create a task"}</button></div>`;
    renderTaskNavigation(); renderSelection(); return;
  } else if (view === "board") {
    const columns = Object.entries(statuses).filter(([state]) => state !== "cancelled" || $("#show-cancelled").checked || $("#status-filter").value === "cancelled");
    $("#tasks").innerHTML = `<div class="board ${columns.length === 4 ? "four" : ""}">${columns.map(([state, label]) => {
      const cards = visible.filter((task) => task.state === state);
      return `<section class="column" data-drop="${state}" aria-label="${label}"><div class="column-heading"><span class="state-mark ${state}"></span>${label}<span class="count">${cards.length}</span></div>${cards.map((task) => `<article class="card ${active === task.id ? "active" : ""}" data-task="${task.id}" draggable="${!held(task) && !dirty(task.id)}"><div class="card-top"><input type="checkbox" data-select="${task.id}" aria-label="Select task ${task.num}" ${selected.has(task.id) ? "checked" : ""}><span class="task-num">Task ${task.num}</span></div><button class="card-title" data-open="${task.id}">${escapeHtml(task.title)}</button>${task.body ? `<p class="card-description">${escapeHtml(task.body)}</p>` : ""}<div class="card-meta">${taskBadges(task)}</div><div class="card-bottom">${stateSelect(task)}<span class="export-label">${escapeHtml(exportLabel(task))}</span></div></article>`).join("") || '<p class="empty-column">No tasks here yet</p>'}</section>`;
    }).join("")}</div>`;
  } else {
    $("#tasks").innerHTML = $("#group-by").value === "state" ? Object.entries(statuses).map(([state,label]) => {
      const items = visible.filter(task => task.state === state);
      return items.length ? `<section class="task-group" aria-label="${label}"><div class="group-heading"><span class="state-mark ${state}"></span><h2>${label}</h2><span>${items.length}</span></div>${items.map(renderRow).join("")}</section>` : "";
    }).join("") : `<div class="task-list">${visible.map(renderRow).join("")}</div>`;
    document.querySelectorAll(".edit-row").forEach((row) => {
      if (drafts.get(row.dataset.task)?.saving) row.querySelectorAll("input,select,button").forEach((input) => { input.disabled = true; });
    });
  }
  if (pendingNew) $("#tasks").innerHTML = pendingNew + $("#tasks").innerHTML;
  $("#tasks").scrollTop = listScroll;
  if (focusAttribute) $("#tasks").querySelector(`[${focusAttribute}="${CSS.escape(focusValue)}"]`)?.focus({preventScroll:true});
  renderTaskNavigation();
  renderSelection();
}

function isRequired(field) { return taskFormat.required_fields.includes(field.field); }
// The display names of required fields this draft leaves unset. Closed
// tasks are history, not work to shape: the CLI doesn't check them either.
function missingRequired(draft) {
  if (["done", "cancelled"].includes(draft.values.state)) return [];
  return fields.filter((field) => isRequired(field) && !(draft.values.fields[field.field] || "").trim()).map((field) => field.display_name);
}
function formatGuide() {
  return taskFormat.guide ? `<details class="format-guide"><summary>How this project writes tasks</summary><p class="help">${escapeHtml(taskFormat.guide.trim()).replace(/\n/g,"<br>")}</p></details>` : "";
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

function renderInlineRow(task) {
  const d = inline === task.id ? draftFor(task.id) : null;
  const value = (key) => escapeHtml(d.values[key]);
  return `<tr data-task="${task.id}" class="${d ? "edit-row" : ""}"><td><input type="checkbox" data-select="${task.id}" aria-label="Select task ${task.num}" ${selected.has(task.id) ? "checked" : ""}></td><td class="title-cell"><span class="task-num">Task ${task.num}</span>${d ? `<input class="title-input" data-input="title" aria-label="Task title" value="${value("title")}">` : `<button class="card-title row-title" data-open="${task.id}">${escapeHtml(task.title)}</button>`}<div class="row-sub">${task.blocked ? badge("Waiting on tasks","blocked") : ""}${held(task) ? badge(task.lease.holder,"holder") : ""}${dirty(task.id) ? badge("Unsaved","draft") : ""}</div></td><td>${stateSelect(task,d)}</td><td>${d ? `<select data-input="priority" aria-label="Priority">${options(priorities,d.values.priority)}</select>` : badge(priorities[task.priority],task.priority)}</td><td>${d ? `<input data-input="assignee" aria-label="Assignee" value="${value("assignee")}">` : escapeHtml(task.assignee || "—")}</td><td>${d ? `<input data-input="labels" aria-label="Labels, comma separated" value="${value("labels")}">` : task.labels.map((tag) => badge(tag)).join(" ") || "—"}</td>${fields.map((field) => `<td>${d ? fieldControl(field,d.values.fields[field.field] || "") : escapeHtml(task.fields[field.field] || "—")}</td>`).join("")}<td>${badge(exportLabel(task),exportState(task))}</td><td><div class="row-actions">${d ? `<button data-save="${task.id}" class="primary" ${d.saving ? "disabled" : ""}>${d.saving ? "Saving…" : "Save"}</button><button data-discard="${task.id}" ${d.saving ? "disabled" : ""}>Cancel</button>` : `<button data-inline="${task.id}">Edit</button>`}</div></td></tr>${d?.error ? `<tr class="row-error"><td colspan="${8 + fields.length}"><p role="alert">${escapeHtml(d.error)}</p><button data-open="${task.id}">Open task to compare</button></td></tr>` : ""}`;
}

function renderRow(task) {
  if (inline === task.id) return `<div class="inline-table"><table><tbody>${renderInlineRow(task)}</tbody></table></div>`;
  return `<div class="task-row ${active === task.id ? "active" : ""} ${selected.has(task.id) ? "selected" : ""}" data-task="${task.id}"><input type="checkbox" data-select="${task.id}" aria-label="Select task ${task.num}" ${selected.has(task.id) ? "checked" : ""}><span class="state-mark ${task.state}" title="${statuses[task.state]}" aria-label="${statuses[task.state]}"></span><span class="task-num">#${task.num}</span><button class="row-title" data-open="${task.id}" title="${escapeHtml(task.title)}" ${active === task.id ? 'aria-current="true"' : ""}>${escapeHtml(task.title)}</button><span class="row-indicators">${held(task) ? `<span class="holder-indicator" title="${escapeHtml(`${task.lease.holder} is working on this task and controls its status`)}">${escapeHtml(task.lease.holder)}</span>` : ""}${dirty(task.id) ? '<span class="draft-dot" title="Unsaved changes" aria-label="Unsaved changes"></span>' : ""}${task.blocked ? '<span class="waiting-icon" title="Waiting on other tasks" aria-label="Waiting on other tasks">◷</span>' : ""}${task.media?.length ? `<span class="attachment-indicator" title="${task.media.length} attachments">▧ ${task.media.length}</span>` : ""}</span><span class="row-tags">${task.labels.slice(0,2).map(tag=>badge(tag)).join("")}${task.labels.length>2 ? `<span class="more-tags" title="${escapeHtml(task.labels.join(", "))}">+${task.labels.length-2}</span>` : ""}</span><span class="row-priority"><span class="priority-dot ${task.priority}"></span>${priorities[task.priority]}</span><span class="owner-avatar ${task.assignee ? "assigned" : ""}" title="${escapeHtml(task.assignee || "Unassigned")}">${escapeHtml(task.assignee?.slice(0,2).toUpperCase() || "–")}</span><button class="row-edit quiet-button" data-inline="${task.id}" aria-label="Quick edit task ${task.num}" title="Quick edit">✎</button></div>`;
}

function renderSelection() {
  const visible = visibleTasks(), count = visible.filter((task) => selected.has(task.id)).length;
  const pendingNew = workspaceScope === "drafts" && drafts.has("new") ? 1 : 0;
  const hidden = selected.size - count;
  $("#select-visible").checked = visible.length > 0 && count === visible.length;
  $("#select-visible").indeterminate = count > 0 && count < visible.length;
  $("#select-visible").disabled = !visible.length || bulkBusy;
  $("#selection-label").textContent = selected.size ? `${selected.size} selected${hidden ? ` · ${hidden} outside this view` : ""}` : "Select all";
  $("#clear-selection").hidden = !selected.size;
  $("#task-count").textContent = workspaceScope === "drafts" ? `${visible.length + pendingNew} unsaved ${visible.length + pendingNew === 1 ? "draft" : "drafts"}` : `${visible.length} of ${tasks.length} tasks`;
  $("#export-count").textContent = selected.size;
  $("#export-button").disabled = !selected.size || !connected || bulkBusy;
  const countDrafts = dirtyIds().length;
  $("#draft-count").textContent = countDrafts ? `${countDrafts} unsaved ${countDrafts === 1 ? "draft" : "drafts"}` : "";
  const saveLabel = $(".save-label");
  if (saveLabel && active) saveLabel.textContent = dirty(active) ? "Unsaved changes" : "All changes saved";
  for (const button of document.querySelectorAll("[data-save]")) button.disabled = bulkTargets.has(button.dataset.save) || Boolean(drafts.get(button.dataset.save)?.saving || drafts.get(button.dataset.save)?.upload);
  for (const checkbox of document.querySelectorAll("[data-select]")) { checkbox.checked=selected.has(checkbox.dataset.select); checkbox.disabled=bulkBusy; }
  for (const row of document.querySelectorAll(".task-row,.card")) row.classList.toggle("selected",selected.has(row.dataset.task));
  // The bulk controls replace the counts in the same row, so selecting a task
  // never moves the list under the pointer.
  const bulkOpen=Boolean(selected.size || bulkBusy);
  $("#bulk-bar").hidden=!bulkOpen;
  $(".selection-bar").classList.toggle("has-selection",bulkOpen);
  $("#bulk-apply").textContent=bulkBusy ? "Applying…" : `Apply to ${selected.size} ${selected.size === 1 ? "task" : "tasks"}`;
  $("#bulk-apply").disabled=bulkBusy || !connected || !selected.size || !bulkReady(bulkChoice());
  for (const control of [$("#bulk-field"),$("#bulk-value"),$("#clear-selection")]) if (control) control.disabled=bulkBusy;
  $("#bulk-note").textContent=$("#bulk-note").title=bulkBusy ? "" : bulkNote();
  $("#heading-count").textContent=visible.length + pendingNew;
  const scopes={all:"All tasks",active:"Active tasks",done:"Completed",blocked:"Waiting",drafts:"Unsaved drafts"};
  $("#view-title").textContent=scopes[workspaceScope];
  const counts={all:tasks.filter(t=>t.state!=="cancelled").length,active:tasks.filter(t=>["todo","doing"].includes(t.state)).length,done:tasks.filter(t=>t.state==="done").length,blocked:tasks.filter(t=>t.blocked).length,drafts:countDrafts};
  for (const button of document.querySelectorAll("[data-scope]")) button.setAttribute("aria-current",button.dataset.scope === workspaceScope ? "page" : "false");
  for (const count of document.querySelectorAll("[data-scope-count]")) count.textContent=counts[count.dataset.scopeCount];
  const filterCount=["#status-filter","#priority-filter","#label-filter","#export-filter"].filter(id=>$(id).value).length+Object.values(fieldFilters).filter(Boolean).length+Number($("#show-cancelled").checked);
  $("#filter-count").textContent=filterCount || "";
  $("#filter-toggle").classList.toggle("filtered",Boolean(filterCount));
}

function renderEditor() {
  clearDescriptionPreview();
  $("#editor").hidden = !active;
  $("#workspace").classList.toggle("has-editor",Boolean(active));
  if (!active) { $("#editor").innerHTML = ""; return; }
  const d = drafts.get(active), task = taskById(active) || d.base;
  const value = (key) => escapeHtml(d.values[key]);
  $("#editor").innerHTML = `
    <div class="editor-heading"><h2 tabindex="-1" id="editor-heading">${active === "new" ? "New task" : `Task ${task.num}`}</h2><div id="task-navigation" class="task-navigation" aria-label="Review navigation"></div><button data-action="close-editor" aria-label="Close task editor" title="Close · Esc">×</button></div>
    <div class="editor-body"><fieldset class="task-form" ${d.saving || bulkTargets.has(active) ? "disabled" : ""}>
      ${d.error ? `<div class="notice error" role="alert">${escapeHtml(d.error)}</div>` : ""}
      ${d.latest && dirty(active) ? `<div class="notice">Someone updated this task. Your changes are still here.<details open><summary>Compare with the saved task</summary><dl class="comparison"><dt>Title</dt><dd>${escapeHtml(d.latest.title)}</dd><dt>Status / Priority</dt><dd>${statuses[d.latest.state]} / ${priorities[d.latest.priority]}</dd><dt>Owner / Tags</dt><dd>${escapeHtml(d.latest.assignee || "Unassigned")} / ${escapeHtml(d.latest.labels.join(", ") || "None")}</dd>${fields.map((f) => `<dt>${escapeHtml(f.display_name)}</dt><dd>${escapeHtml(d.latest.fields[f.field] || "Not set")}</dd>`).join("")}<dt>Description</dt><dd>${escapeHtml(d.latest.body || "No description")}</dd></dl></details><button data-action="rebase" ${d.upload ? "disabled" : ""}>Keep my edits and review</button></div>` : ""}
      <label class="form-field title-field"><span class="sr-only">Task title</span><textarea rows="1" data-input="title" id="editor-title" placeholder="What needs doing?" ${d.saving ? "disabled" : ""}>${value("title")}</textarea></label>
      <div class="property-row"><label class="form-field"><span>Status</span><select data-input="state" ${held(task) || active === "new" || d.saving ? "disabled" : ""}>${options(statuses,d.values.state)}</select></label><label class="form-field"><span>Priority</span><select data-input="priority" ${d.saving ? "disabled" : ""}>${options(priorities,d.values.priority)}</select></label><label class="form-field"><span>Owner</span><input data-input="assignee" value="${value("assignee")}" placeholder="Unassigned"></label></div>
      ${held(task) ? `<p class="help">${escapeHtml(task.lease.holder)} is working on this task. You can edit its details; they control its status.</p>` : ""}
      ${active === "new" ? formatGuide() : ""}
      <details class="organize-task" ${missingRequired(d).length ? "open" : ""}><summary>Tags & properties <span>${escapeHtml(missingRequired(d).length ? `Required: ${missingRequired(d).join(", ")}` : d.values.labels || "Add details")}</span></summary><label class="form-field"><span>Tags</span><input data-input="labels" value="${value("labels")}" placeholder="Separate tags with commas" ${d.saving ? "disabled" : ""}></label>${fields.map((field) => `<label class="form-field"><span>${escapeHtml(field.display_name)}${isRequired(field) ? ' <em class="required-mark" title="This project\'s task format requires it">required</em>' : ""}</span>${fieldControl(field,d.values.fields[field.field] || "",d.saving ? "disabled" : "")}</label>`).join("")}</details>
      <div class="task-tools"><button data-action="add-media" class="quiet-button">+ Attach screenshot</button>${task.media?.length ? `<button data-action="show-attachments" class="quiet-button">▧ ${task.media.length} attachments ↓</button>` : ""}</div>
      ${descriptionEditor(d)}
      ${screenshotsEditor()}
      ${active !== "new" ? `<details><summary>Related tasks & history</summary><div id="task-history">Loading history…</div></details>` : ""}
    </fieldset></div>
    <div class="editor-actions"><span class="save-label" role="status">${dirty(active) ? "Unsaved changes" : "All changes saved"}</span><button data-discard="${active}" class="quiet-button" ${d.saving || bulkTargets.has(active) ? "disabled" : ""}>Discard</button><button data-save="${active}" class="primary" ${d.saving ? "disabled" : ""}>${d.saving ? "Saving…" : "Save"}</button>${active !== "new" ? '<button id="save-next" data-action="save-next">Save & next</button>' : ""}</div>`;
  renderTaskNavigation();
  fitTaskTitle();
  if (active !== "new") loadHistory(active);
  previewDescription();
  renderUploadState(active);
}

function fitTaskTitle() {
  const title = $("#editor-title");
  if (!title) return;
  title.style.height = "auto";
  title.style.height = `${title.scrollHeight + 2}px`;
}

function descriptionEditor(d) {
  const preview = d.descriptionMode === "preview";
  return `<section class="description-field form-field"><div class="description-heading"><label for="description-source">Description</label><div class="view-switch description-switch" role="group" aria-label="Description view"><button type="button" data-description-mode="preview" aria-pressed="${preview}" aria-controls="description-preview">Preview</button><button type="button" data-description-mode="write" aria-pressed="${!preview}" aria-controls="description-source">Edit text</button></div></div><p id="description-help" class="help" ${preview ? "hidden" : ""}>Plain text works here. Use **bold**, - for a list, or - [ ] for a checklist. Preview shows the finished result.</p><textarea id="description-source" data-input="body" aria-describedby="description-help" spellcheck="true" placeholder="What should someone know about this task?" ${d.saving ? "disabled" : ""} ${preview ? "hidden" : ""}>${escapeHtml(d.values.body)}</textarea><div id="description-preview" class="markdown-preview" role="region" aria-label="Description preview" aria-live="polite" tabindex="0" ${preview ? "" : "hidden"}></div></section>`;
}

function screenshotsEditor() {
  return `<section class="screenshots-section" aria-labelledby="screenshots-title"><div class="description-heading"><h3 id="screenshots-title">Screenshots & files <span id="attachment-count"></span></h3><button type="button" id="add-media" data-action="add-media" aria-describedby="media-storage">+ Add screenshots / files</button></div><div id="attachment-gallery" class="attachment-gallery" aria-live="polite"><p class="help">Loading attachments…</p></div><input type="file" id="media-picker" aria-label="Choose screenshots or videos" accept=".png,.jpg,.jpeg,.gif,.webp,.mp4,.mov,.webm,image/png,image/jpeg,image/gif,image/webp,video/mp4,video/quicktime,video/webm" multiple hidden><p class="help">Choose a file, or paste a copied screenshot while this task is open. Click a thumbnail to enlarge it.</p><p id="media-status" class="help" role="status" hidden></p><p id="media-errors" class="media-error" role="alert" hidden></p><details class="attachment-help"><summary>File types & storage</summary><p id="media-formats" class="help">${escapeHtml(mediaHelp())}</p><p id="media-storage" class="help">Files stay on this computer. Save changes to attach them to the task. Discarding changes does not delete the copied files.</p></details></section>`;
}

function clearDescriptionPreview() {
  clearTimeout(descriptionTimer);
  closeMediaViewer();
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
  $("#add-media").disabled = d.saving || Boolean(d.upload) || bulkTargets.has(id);
  $("#add-media").textContent = d.upload ? "Adding files…" : "+ Add screenshots / files";
  $("#description-source").disabled = d.saving || Boolean(d.upload) || bulkTargets.has(id);
  $("#media-status").textContent = d.mediaMessage;
  $("#media-status").hidden = !d.mediaMessage;
  $("#media-errors").textContent = d.mediaErrors.join("\n");
  $("#media-errors").hidden = !d.mediaErrors.length;
  $("#media-formats").textContent = mediaHelp();
  const nextButton = $("#save-next");
  if (nextButton) nextButton.disabled = !adjacentTask(1) || d.saving || Boolean(d.upload) || bulkTargets.has(id);
  renderSelection();
}

function chooseMedia() {
  const d = drafts.get(active);
  if (!d || d.saving || d.upload || bulkTargets.has(active)) return;
  // Capture the draft and insertion point before the native picker opens.
  // Its eventual result must not belong to whichever task is active later.
  const source = $("#description-source");
  mediaPicker = {id:active,d,offset:d.descriptionMode === "write" ? source.selectionEnd : d.values.body.length};
  $("#media-picker").value = "";
  $("#media-picker").click();
}

function pasteScreenshots(event) {
  if (!active || !event.target.closest("#editor")) return;
  const files = [...(event.clipboardData?.items || [])].filter(item => item.kind === "file" && item.type.startsWith("image/")).map(item => item.getAsFile()).filter(Boolean);
  if (!files.length) return;
  const d = drafts.get(active);
  if (!d || d.saving || d.upload || bulkTargets.has(active)) return;
  event.preventDefault();
  const source = $("#description-source");
  addMedia(files,{id:active,d,offset:d.descriptionMode === "write" ? source.selectionEnd : d.values.body.length});
}

function closeMediaViewer() {
  const dialog = $("#media-dialog");
  if (dialog?.open) dialog.close();
}
function openMediaViewer(button) {
  const dialog = $("#media-dialog"), video = button.dataset.mediaType.startsWith("video/");
  const element = document.createElement(video ? "video" : "img");
  const caption = button.dataset.caption || (video ? "Video" : "Screenshot");
  if (video) { element.controls = true; element.setAttribute("aria-label",caption); }
  else element.alt = caption;
  element.src = button.dataset.viewMedia;
  $("#media-viewer-title").textContent = caption;
  $("#media-viewer-content").replaceChildren(element);
  dialog.showModal();
}

function addAttachmentThumbnail(gallery, url, type, caption) {
  const button = document.createElement("button"), video = type.startsWith("video/");
  button.type = "button"; button.className = "attachment-thumbnail";
  button.dataset.viewMedia = url; button.dataset.mediaType = type; button.dataset.caption = caption;
  button.setAttribute("aria-label",`Open ${video ? "video" : "screenshot"}: ${caption || "Attachment"}`);
  if (!video) {
    const image = document.createElement("img"); image.src = url; image.alt = "";
    button.append(image);
  } else {
    const icon = document.createElement("span"); icon.className = "video-thumbnail"; icon.textContent = "▶ Video"; button.append(icon);
  }
  const label = document.createElement("span"); label.textContent = caption || (video ? "Video" : "Screenshot");
  button.append(label); gallery.append(button);
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
          clearDescriptionPreview(); previewDescription();
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
  $("#description-help").hidden = mode === "preview";
  previewDescription();
}

async function previewDescription() {
  const panel = $("#description-preview"), gallery = $("#attachment-gallery"), source = drafts.get(active)?.values.body || "";
  const controller = new AbortController();
  descriptionRequest = controller;
  const current = () => descriptionRequest === controller && panel.isConnected;
  panel.classList.remove("preview-error");
  panel.setAttribute("aria-busy","true");
  panel.textContent = source.trim() ? "Loading description…" : "No description yet. Choose Edit text to add some context.";
  gallery.innerHTML = '<p class="help">No screenshots yet. Add one when a picture would help explain the task.</p>';
  $("#attachment-count").textContent = "";
  try {
    if (!source.trim()) return;
    const result = await api("/api/markdown/preview","POST",{markdown:source},controller.signal);
    if (!current()) return;
    // Only the server's restricted Markdown renderer supplies this HTML.
    panel.innerHTML = result.html;
    const placeholders = [...panel.querySelectorAll("[data-media]")];
    if (placeholders.length) gallery.replaceChildren();
    $("#attachment-count").textContent = placeholders.length ? `(${placeholders.length})` : "";
    const mediaCache = new Map();
    for (const placeholder of placeholders) {
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
        addAttachmentThumbnail(gallery,url,type,caption || "");
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
        const missing = document.createElement("p"); missing.className = "help";
        missing.textContent = `${placeholder.dataset.caption || "Attachment"} could not be loaded. ${error.message}`;
        gallery.append(missing);
      }
    }
  } catch (error) {
    if (!current()) return;
    panel.classList.add("preview-error");
    panel.textContent = `Preview unavailable. ${error.message} Choose Edit text to keep editing.`;
    gallery.textContent = "Attachments could not be loaded. Your description is unchanged.";
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
    $("#task-history").innerHTML = `${blockers.length ? `<p>These tasks come first:</p>${blockers.map((t) => `<button class="related-task" data-open="${t.id}">Task ${t.num} · ${escapeHtml(t.title)} <span>(${statuses[t.state]})</span></button>`).join("")}` : "<p>No tasks need to be finished first.</p>"}<details><summary>Change history</summary><ol>${historyRows.map((event) => `<li>${escapeHtml(event.actor)} · ${escapeHtml(event.type)}${event.applied ? "" : " (no change)"}<br>${escapeHtml(new Date(event.ts).toLocaleString())}<div class="history-value">${escapeHtml(Object.values(event.data || {}).map((v) => typeof v === "object" ? JSON.stringify(v) : v).join(" · "))}</div></li>`).join("")}</ol></details>`;
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
  $("#editor").scrollTop = 0;
  $('#tasks [aria-current="true"]')?.scrollIntoView({block:"nearest"});
  $("#workspace").scrollIntoView({block:"start"});
  $("#editor-heading")?.focus({preventScroll:true});
}
function createTask() {
  if (!drafts.has("new")) {
    const draft = newDraft({id:"new",seq:0,title:"",body:null,state:"todo",priority:"none",assignee:null,labels:[],fields:{},relations:[]});
    draft.values.body = taskFormat.template || "";
    drafts.set("new",draft);
  }
  openTask("new"); $("#editor-title").focus();
}
function discard(id) {
  if (drafts.get(id)?.saving || bulkTargets.has(id)) return;
  drafts.get(id)?.upload?.abort();
  drafts.delete(id);
  if (inline === id) inline = null;
  if (active === id) active = null;
  renderTasks(); renderEditor();
}

async function saveDraft(id) {
  const d = drafts.get(id);
  if (!d || d.saving || d.upload || bulkTargets.has(id)) return false;
  if (id !== "new" && !dirty(id)) { toast("No changes to save."); return true; }
  d.saving = true; d.error = "";
  if (active === id) renderEditor();
  if (inline === id) renderTasks();
  try {
    const payload = id === "new" ? {title:d.values.title,priority:d.values.priority,body:d.values.body.trim() ? d.values.body : null,assignee:d.values.assignee.trim() || null,labels:tags(d.values.labels),fields:Object.fromEntries(Object.entries(d.values.fields).filter(([,v]) => v.trim()))} : patchFor(d);
    const saved = await write(id === "new" ? "/api/tasks" : `/api/task/${id}`,id === "new" ? "POST" : "PATCH",payload);
    const index = tasks.findIndex((task) => task.id === saved.id);
    if (index < 0) tasks.push(saved); else tasks[index] = saved;
    drafts.delete(id);
    if (active === id) { active = saved.id; drafts.set(saved.id,newDraft(saved)); }
    if (inline === id) inline = null;
    toast(id === "new" ? "Task created." : "Changes saved to HippoTask.");
    await refresh(); renderTasks(); renderEditor();
    return true;
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
  return false;
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
  if (bulkBusy) return;
  const task = taskById(id);
  if (!task || task.state === state) return;
  if (dirty(id) || held(task)) { toast(dirty(id) ? "Save or discard this task's draft before moving it." : "The holder controls this task's status.",true); renderTasks(); return; }
  try {
    const saved = await write(`/api/task/${id}`,"PATCH",{base:task.seq,state});
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
    const result = await write("/api/export/save","POST",{ids:preview.ids,again:preview.again,review:preview.review,path:$("#export-path").value});
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

function clearSelection() {
  if (bulkBusy) return;
  selected.clear(); lastSelected=null; renderSelection();
}

function closeEditor() {
  const id=active; active=null; renderEditor(); renderTasks();
  $(`#tasks [data-open="${CSS.escape(id || "")}"]`)?.focus({preventScroll:true});
}

function resetFilters() {
  for (const selector of ["#search","#status-filter","#priority-filter","#label-filter","#export-filter"]) $(selector).value="";
  $("#show-cancelled").checked=false; fieldFilters={};
  for (const field of document.querySelectorAll("[data-filter-field]")) field.value="";
  renderTasks();
}

document.addEventListener("click", (event) => {
  if (event.target.dataset.select) { selectTask(event.target.dataset.select,event.target.checked,event.shiftKey); return; }
  const button = event.target.closest("button");
  if (!button || button.disabled) return;
  if (button.dataset.open) openTask(button.dataset.open);
  if (button.dataset.inline && !bulkBusy) {
    const id = button.dataset.inline;
    if (!dirty(id)) drafts.set(id,newDraft(taskById(id)));
    inline = id; if (active === id) active = null;
    renderEditor(); renderTasks();
    $(`tr[data-task="${id}"] .title-input`)?.focus();
  }
  if (button.dataset.save) saveDraft(button.dataset.save);
  if (button.dataset.discard) discard(button.dataset.discard);
  if (button.dataset.descriptionMode) setDescriptionMode(button.dataset.descriptionMode);
  if (button.dataset.navigate) navigateTask(Number(button.dataset.navigate));
  if (button.dataset.viewMedia) openMediaViewer(button);
  if (button.dataset.action === "save-next") saveAndNext();
  if (button.dataset.action === "add-media") chooseMedia();
  if (button.dataset.action === "show-attachments") $(".screenshots-section")?.scrollIntoView({block:"start",behavior:"smooth"});
  if (button.dataset.action === "undo-bulk") undoBulkChange();
  if (button.dataset.action === "dismiss-bulk") { bulkResult=null; renderBulkResult(); }
  if (button.dataset.action === "reset-filters") { workspaceScope="all"; resetFilters(); }
  if (button.dataset.scope) { workspaceScope=button.dataset.scope; $("#status-filter").value=""; renderTasks(); }
  if (button.dataset.action === "new") createTask();
  if (button.dataset.action === "close-editor") closeEditor();
  if (button.dataset.action === "rebase") rebaseDraft();
});
function inputDraft(event) {
  const input = event.target;
  if (!input.hasAttribute("data-input") && !input.hasAttribute("data-input-field")) return;
  const id = input.closest("#editor") ? active : input.closest("tr[data-task]")?.dataset.task;
  const d = drafts.get(id);
  if (!d || d.saving || bulkTargets.has(id)) return;
  if (input.hasAttribute("data-input-field")) d.values.fields[input.dataset.inputField] = input.value;
  else d.values[input.dataset.input] = input.value;
  if (input.id === "editor-title") fitTaskTitle();
  if (input.dataset.input === "body" && id === active) {
    clearDescriptionPreview();
    descriptionTimer = setTimeout(previewDescription,400);
  }
  renderSelection();
}
document.addEventListener("paste",pasteScreenshots);
document.addEventListener("input",inputDraft);
document.addEventListener("change", (event) => {
  const input = event.target;
  if (input.id === "task-chooser") { if (input.value) openTask(input.value); return; }
  if (input.id === "media-picker") { const selection = mediaPicker; mediaPicker = null; addMedia([...input.files],selection); return; }
  inputDraft(event);
  if (input.dataset.move) moveTask(input.dataset.move,input.value);
  if (input.hasAttribute("data-filter-field")) { fieldFilters[input.dataset.filterField] = input.value; renderTasks(); }
});
document.addEventListener("keydown", (event) => {
  if (event.defaultPrevented || $("#media-dialog").open || $("#export-dialog").open || $("#shortcuts-dialog").open) return;
  // A focused checkbox (the usual state after clicking one) isn't typing.
  const editing = event.target.closest("input:not([type=checkbox]):not([type=radio]),textarea,select,[contenteditable=true]");
  if (active && !editing && event.altKey && ["ArrowLeft","ArrowRight"].includes(event.key)) {
    event.preventDefault(); navigateTask(event.key === "ArrowLeft" ? -1 : 1); return;
  }
  if (active && !editing && event.key === "Escape") {
    event.preventDefault(); closeEditor(); return;
  }
  if (!editing && !event.metaKey && !event.ctrlKey && !event.altKey && event.key === "/") { event.preventDefault(); $("#search").focus(); return; }
  if (!editing && !event.metaKey && !event.ctrlKey && !event.altKey && event.key.toLowerCase() === "c") { event.preventDefault(); createTask(); return; }
  if (!editing && event.key === "Escape") clearSelection();
  if (!editing && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a" && event.target.closest("#tasks,.selection-bar") && !bulkBusy) { event.preventDefault(); reviewTasks().forEach(task=>selected.add(task.id)); renderSelection(); }
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
$("#select-visible").onchange = (event) => { if (bulkBusy) return; for (const task of visibleTasks()) { if (event.target.checked) selected.add(task.id); else selected.delete(task.id); } renderSelection(); };
$("#clear-selection").onclick = clearSelection;
$("#bulk-field").onchange = renderBulkValue;
$("#bulk-value-control").addEventListener("input",renderSelection);
$("#bulk-value-control").addEventListener("change",renderSelection);
$("#bulk-apply").onclick = () => runBulkChange(bulkChoice());
$("#filter-toggle").onclick = () => { $("#filters").hidden=!$("#filters").hidden; $("#filter-toggle").setAttribute("aria-expanded",String(!$("#filters").hidden)); };
$("#reset-filters").onclick = resetFilters;
$("#sort-order").onchange = renderTasks;
$("#group-by").onchange = renderTasks;
$("#show-shortcuts").onclick = () => $("#shortcuts-dialog").showModal();
$("#close-shortcuts").onclick = () => $("#shortcuts-dialog").close();
$("#search").oninput = renderTasks;
for (const selector of ["#status-filter","#priority-filter","#label-filter","#export-filter","#show-cancelled"]) $(selector).onchange = renderTasks;
$("#close-export").onclick = () => { if (!exporting) { previewVersion++; $("#export-dialog").close(); } };
$("#export-dialog").addEventListener("cancel", (event) => { if (exporting) event.preventDefault(); else previewVersion++; });
$("#include-again").onchange = previewExport;
$("#refresh-preview").onclick = previewExport;
$("#save-export").onclick = saveExport;
$("#close-media").onclick = closeMediaViewer;
$("#media-dialog").addEventListener("close", () => $("#media-viewer-content").replaceChildren());
window.addEventListener("beforeunload", (event) => { if (dirtyIds().length || exporting || bulkBusy) { event.preventDefault(); event.returnValue = ""; } });
window.addEventListener("focus",refresh);
setInterval(() => { if (!document.hidden) refresh(); },5000);
refresh();
