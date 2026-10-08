const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

// A stand-in for event.target: enough of closest() for the app's selectors
// (tag, tag:not([type=…]), and ancestors named by `inside`).
function target({tag='input', type='checkbox', inside=[]}={}) {
  const matches = (selectors) => selectors.split(',').some((part) => {
    part = part.trim();
    if (inside.includes(part)) return true;
    const simple = part.match(/^([a-z]+)((?::not\(\[type=[a-z]+\]\))*)$/);
    if (!simple || simple[1] !== tag) return false;
    return ![...simple[2].matchAll(/type=([a-z]+)/g)].some((excluded) => excluded[1] === type);
  });
  return {tagName:tag.toUpperCase(), type, closest:(selectors) => matches(selectors) ? {} : null};
}
function key(name, eventTarget, modifiers = {}) {
  return {key:name, target:eventTarget, metaKey:false, ctrlKey:false, altKey:false, shiftKey:false,
    defaultPrevented:false, preventDefault() { this.defaultPrevented = true; }, ...modifiers};
}

function app() {
  const listeners = {};
  const elements = new Map();
  const element = (selector) => {
    if (!elements.has(selector)) elements.set(selector, {
      value:'', checked:false, hidden:false, dataset:{}, textContent:'', innerHTML:'',
      classList:{add(){},remove(){},toggle(){}}, setAttribute(){}, addEventListener(){},
      querySelector(){return null;}, querySelectorAll(){return [];}, focus(){}, scrollIntoView(){},
    });
    return elements.get(selector);
  };
  const context = vm.createContext({
    document:{querySelector:element,querySelectorAll:()=>[],addEventListener(type,listener){(listeners[type] ||= []).push(listener);},activeElement:null},
    window:{addEventListener(){}}, location:{hash:''}, history:{replaceState(){}},
    sessionStorage:{getItem(){return '';},setItem(){}}, structuredClone, AbortController,
    URL, console, setInterval(){},setTimeout(){},clearTimeout(){},CSS:{escape:x=>x},
    fetch:()=>new Promise(()=>{}),
  });
  vm.runInContext(fs.readFileSync(path.join(__dirname,'app.js'),'utf8'),context);
  vm.runInContext(`
    tasks = [1,2,3].map(num => ({id:String(num),num,seq:1,title:'Task '+num,
      body:'Description '+num,state:num===2?'doing':'todo',priority:'none',
      assignee:null,labels:[],fields:{},relations:[],exported:{}}));
    view='list'; connected=true;
    var renderRealTasks=renderTasks, realRefresh=refresh;
    renderTasks=()=>{}; renderEditor=()=>{}; renderSelection=()=>{};
    refresh=async()=>{}; toast=()=>{};
  `,context);
  const dispatch = (type, event) => { for (const listener of listeners[type] || []) listener(event); return event; };
  return {run:code=>vm.runInContext(code,context), element, context, dispatch};
}

test('existing descriptions open formatted; blank new tasks open for writing',()=>{
  const a=app();
  assert.equal(a.run('newDraft(tasks[0]).descriptionMode'),'preview');
  assert.equal(a.run('newDraft({...tasks[0],body:null}).descriptionMode'),'write');
});

test('opening a task brings its editor header and selected list item into view',()=>{
  const a=app(); const revealed=[];
  a.element('#tasks [aria-current="true"]').scrollIntoView=()=>revealed.push('task');
  a.element('#workspace').scrollIntoView=()=>revealed.push('editor');
  a.run('openTask("3")');
  assert.deepEqual(revealed,['task','editor']);
});

test('previous/next follow the filtered list and stop at its ends',()=>{
  const a=app(); a.run('openTask("1")');
  a.run('navigateTask(-1)'); assert.equal(a.run('active'),'1');
  a.run('navigateTask(1)'); assert.equal(a.run('active'),'2');
  a.element('#status-filter').value='todo';
  a.run('navigateTask(1)'); assert.equal(a.run('active'),'2','filtered-out current task stays open');
  a.run('openTask("1"); navigateTask(1)'); assert.equal(a.run('active'),'3');
  a.run('navigateTask(1)'); assert.equal(a.run('active'),'3');
});

test('board review order follows columns, and drafts survive a round trip',()=>{
  const a=app(); a.run('view="board"; openTask("1"); drafts.get("1").values.body="Unsaved edit"; navigateTask(1)');
  assert.equal(a.run('active'),'3');
  a.run('navigateTask(-1)');
  assert.equal(a.run('drafts.get("1").values.body'),'Unsaved edit');
  assert.equal(a.run('dirty("1")'),true);
  assert.equal(a.run('tasks[0].body'),'Description 1');
});

test('Save & next moves after success even when saving changes filter membership',async()=>{
  const a=app(); a.element('#status-filter').value='todo';
  a.run('openTask("1"); drafts.get("1").values.state="done"; api=async()=>({...tasks[0],state:"done",seq:2})');
  await a.run('saveAndNext()');
  assert.equal(a.run('active'),'3');
  assert.equal(a.run('dirty("1")'),false);
  assert.equal(a.run('tasks[0].state'),'done');
});

test('a failed or stale Save & next retains the current task and draft',async()=>{
  for (const kind of ['connection','stale']) {
    const a=app();
    a.run(`openTask("1"); drafts.get("1").values.title="My edit";
      api=async(path,method)=>{if(method==='PATCH') throw Object.assign(new Error('Try again'),{kind:'${kind}'});return {...tasks[0],seq:2};}`);
    await a.run('saveAndNext()');
    assert.equal(a.run('active'),'1');
    assert.equal(a.run('drafts.get("1").values.title'),'My edit');
    assert.equal(a.run('dirty("1")'),true);
  }
});

test('late save completion does not navigate away from another task',async()=>{
  const a=app();
  a.run('openTask("1"); drafts.get("1").values.title="Saved edit"; var finishSave; api=()=>new Promise(resolve=>finishSave=resolve)');
  const saving=a.run('saveAndNext()');
  a.run('openTask("3"); finishSave({...tasks[0],title:"Saved edit",seq:2})');
  await saving;
  assert.equal(a.run('active'),'3');
});

test('Save & next keeps upload drafts in place; no-edit navigation makes no write',async()=>{
  const a=app();
  a.run('var calls=0; api=async()=>{calls++;}; openTask("1"); drafts.get("1").upload=new AbortController()');
  await a.run('saveAndNext()');
  assert.equal(a.run('active'),'1'); assert.equal(a.run('calls'),0);
  a.run('drafts.get("1").upload=null');
  await a.run('saveAndNext()');
  assert.equal(a.run('active'),'2'); assert.equal(a.run('calls'),0);
});

test('screenshot paste uses the active draft, while text and outside pastes pass through',()=>{
  const a=app();
  a.run(`openTask('1'); var captured=null; addMedia=(files,selection)=>{captured=selection;};
    var prevented=false;
    var paste={target:{closest:()=>true},preventDefault(){prevented=true;},
      clipboardData:{items:[{kind:'file',type:'image/png',getAsFile:()=>({name:'image.png'})}]}};
    pasteScreenshots(paste);`);
  assert.equal(a.run('prevented'),true);
  assert.equal(a.run('captured.id'),'1');
  a.run('openTask("2")'); assert.equal(a.run('captured.d.base.id'),'1');
  a.run('prevented=false; paste.clipboardData.items=[]; pasteScreenshots(paste)');
  assert.equal(a.run('prevented'),false);
  a.run('paste.target.closest=()=>null; pasteScreenshots(paste)');
  assert.equal(a.run('prevented'),false);
});

test('Shift-select follows the displayed order and leaves hidden tasks alone',()=>{
  const a=app(); a.element('#status-filter').value='todo';
  a.run('selectTask("1",true); selectTask("3",true,true)');
  assert.equal(a.run('JSON.stringify([...selected])'),'["1","3"]');
  a.run('selectTask("1",false,true)');
  assert.equal(a.run('selected.size'),0);
});

test('sorting changes review order without changing the underlying collection',()=>{
  const a=app(); a.run('tasks[2].priority="urgent"; tasks[1].priority="high"');
  a.element('#sort-order').value='priority';
  assert.equal(a.run('reviewTasks().map(t=>t.id).join(",")'),'3,2,1');
  assert.equal(a.run('tasks.map(t=>t.id).join(",")'),'1,2,3');
});

test('opening a task preserves checkboxes and the main list',()=>{
  const a=app(); a.run('active="2"; renderRealTasks()');
  const html=a.element('#tasks').innerHTML;
  assert.match(html,/data-select="1"/);
  assert.match(html,/data-select="2"/);
  assert.match(html,/data-select="3"/);
});

test('bulk status writes exactly the captured selection with conditional bases',async()=>{
  const a=app();
  a.run(`selected.add('1'); selected.add('3'); var sent=[];
    api=async(path,method,patch)=>{sent.push({path,method,patch});const task=taskById(path.split('/').pop());return {...task,...patch,seq:task.seq+1};};`);
  a.element('#status-filter').value='doing';
  await a.run('runBulkChange({field:"state",value:"done"})');
  assert.equal(a.run('sent.length'),2);
  assert.equal(a.run('sent.map(x=>x.path).join(",")'),'/api/task/1,/api/task/3');
  assert.equal(a.run('sent.every(x=>x.method==="PATCH" && x.patch.base===1 && x.patch.state==="done")'),true);
  assert.equal(a.run('tasks[1].state'),'doing');
  assert.equal(a.run('bulkResult.updated.length'),2);
  assert.equal(a.run('selected.size'),2,'selection remains available for another edit');
});

test('bulk edits skip local drafts and held status changes, but update other tasks',async()=>{
  const a=app();
  a.run(`tasks.forEach(t=>selected.add(t.id)); openTask('1'); drafts.get('1').values.body='My draft';
    tasks[1].lease={active:true,holder:'agent:other'}; var sent=[];
    api=async(path,method,patch)=>{sent.push(path);return {...tasks[2],...patch,seq:2};};`);
  await a.run('runBulkChange({field:"state",value:"done"})');
  assert.equal(a.run('sent.join(",")'),'/api/task/3');
  assert.equal(a.run('drafts.get("1").values.body'),'My draft');
  assert.equal(a.run('bulkResult.failed.length'),2);
  assert.equal(a.run('bulkResult.updated.length'),1);
});

test('partial stale failures are reported without undoing successful edits',async()=>{
  const a=app();
  a.run(`tasks.forEach(t=>selected.add(t.id));
    api=async(path,method,patch)=>{const t=taskById(path.split('/').pop());if(t.id==='2')throw Object.assign(new Error('Updated elsewhere'),{kind:'stale'});return {...t,...patch,seq:2};};`);
  await a.run('runBulkChange({field:"priority",value:"high"})');
  assert.equal(a.run('bulkResult.updated.map(t=>t.id).join(",")'),'1,3');
  assert.equal(a.run('bulkResult.failed[0].id'),'2');
  assert.equal(a.run('tasks[1].priority'),'none');
  assert.equal(a.run('selected.size'),3);
});

test('an uncertain connection stops the batch and does not retry or claim success',async()=>{
  const a=app();
  a.run(`tasks.forEach(t=>selected.add(t.id)); var sent=0;
    api=async()=>{sent++;throw Object.assign(new Error('Offline'),{kind:'connection'});};`);
  await a.run('runBulkChange({field:"priority",value:"high"})');
  assert.equal(a.run('sent'),1);
  assert.equal(a.run('bulkResult.updated.length'),0);
  assert.equal(a.run('bulkResult.failed.length'),3);
  assert.match(a.run('bulkResult.failed[0].message'),/not confirmed/i);
  assert.match(a.run('bulkResult.failed[1].message'),/not attempted/i);
});

test('bulk tags are additive/removable, owner clearing is explicit, and no-ops write nothing',async()=>{
  const a=app();
  a.run(`tasks[0].labels=['keep']; tasks[0].assignee='Sam'; selected.add('1'); var sent=[];
    api=async(path,method,patch)=>{sent.push(patch);return {...tasks[0],seq:tasks[0].seq+1,
      labels:[...tasks[0].labels.filter(l=>!(patch.label_remove||[]).includes(l)),...(patch.label_add||[])],
      assignee:patch.unassign?null:tasks[0].assignee};};`);
  await a.run('runBulkChange({field:"add-tags",value:"keep, review, review"})');
  assert.equal(a.run('JSON.stringify(sent[0].label_add)'),'["review"]');
  assert.equal(a.run('tasks[0].labels.join(",")'),'keep,review');
  await a.run('runBulkChange({field:"remove-tags",value:"review"})');
  assert.equal(a.run('tasks[0].labels.join(",")'),'keep');
  await a.run('runBulkChange({field:"unassign"})');
  assert.equal(a.run('sent[2].unassign'),true);
  await a.run('runBulkChange({field:"priority",value:"none"})');
  assert.equal(a.run('sent.length'),3);
  assert.equal(a.run('bulkResult.unchanged.length'),1);
});

test('undo uses the saved sequence and never overwrites subsequent changes',async()=>{
  const a=app();
  a.run(`selected.add('1'); selected.add('2'); var sent=[];
    api=async(path,method,patch)=>{const t=taskById(path.split('/').pop());sent.push(patch);
      if(patch.base!==t.seq)throw Object.assign(new Error('Updated elsewhere'),{kind:'stale'});
      return {...t,...patch,seq:t.seq+1};};`);
  await a.run('runBulkChange({field:"priority",value:"high"})');
  a.run('tasks[1]={...tasks[1],seq:3,priority:"urgent"}');
  await a.run('undoBulkChange()');
  assert.equal(a.run('tasks[0].priority'),'none');
  assert.equal(a.run('tasks[1].priority'),'urgent');
  assert.equal(a.run('sent[2].base'),2);
  assert.equal(a.run('bulkResult.failed[0].id'),'2');
  assert.match(a.run('bulkResult.failed[0].message'),/undo left it/i);
  assert.doesNotMatch(a.run('bulkResult.failed[0].message'),/try again/i,'an undo cannot be retried, so do not suggest it');
});

test('bulk operation captures versions before awaiting and rejects double submission',async()=>{
  const a=app();
  a.run(`selected.add('1'); selected.add('2'); var finishFirst, sent=[];
    api=(path,method,patch)=>{sent.push(patch);return sent.length===1?new Promise(r=>finishFirst=r):Promise.resolve({...tasks[1],...patch,seq:3});};`);
  const pending=a.run('runBulkChange({field:"priority",value:"high"})');
  await a.run('runBulkChange({field:"state",value:"done"})');
  a.run('tasks[1]={...tasks[1],seq:2}; selected.add("3"); finishFirst({...tasks[0],priority:"high",seq:2})');
  await pending;
  assert.equal(a.run('sent.length'),2);
  assert.equal(a.run('sent[1].base'),1,'no silent rebase during a batch');
  assert.equal(a.run('tasks[2].priority'),'none');
});

test('the drafts view can reopen a new unsaved task',()=>{
  const a=app(); a.run('createTask(); active=null; workspaceScope="drafts"; renderRealTasks()');
  assert.match(a.element('#tasks').innerHTML,/data-open="new"/);
});

test('bulk fields validate declared choices before writing and support clearing',async()=>{
  const a=app();
  a.run(`fields=[{field:'area',display_name:'Area',values:['Product','Engineering']}]; tasks[0].fields={area:'Product'}; selected.add('1'); var sent=[];
    api=async(path,method,patch)=>{sent.push(patch);return {...tasks[0],seq:2,fields:patch.fields || {}};};`);
  await a.run('runBulkChange({field:"field:area",value:"Invented"})');
  assert.equal(a.run('sent.length'),0);
  await a.run('runBulkChange({field:"field:area",value:"Engineering"})');
  assert.equal(a.run('sent[0].fields.area'),'Engineering');
  await a.run('runBulkChange({field:"clear:area"})');
  assert.equal(a.run('sent[1].clear_fields[0]'),'area');
});

test('held tasks still allow bulk changes to details',async()=>{
  const a=app();
  a.run(`tasks[0].lease={active:true,holder:'agent:other'}; selected.add('1');
    api=async(path,method,patch)=>({...tasks[0],...patch,seq:2});`);
  await a.run('runBulkChange({field:"priority",value:"high"})');
  assert.equal(a.run('bulkResult.updated.length'),1);
  assert.equal(a.run('tasks[0].priority'),'high');
  assert.equal(a.run('tasks[0].lease.holder'),'agent:other');
});

test('an empty filter refreshes the open task navigation',()=>{
  const a=app(); a.run('openTask("1")'); a.element('#status-filter').value='cancelled';
  a.run('renderRealTasks()');
  assert.match(a.element('#task-navigation').innerHTML,/Outside this view/);
});

test('a blank owner or field value never clears; Unassign and Clear are explicit choices',async()=>{
  const a=app();
  a.run(`fields=[{field:'area',display_name:'Area',values:['Product','Engineering']}];
    tasks[0].assignee='Sam'; tasks[0].fields={area:'Product'}; selected.add('1'); var sent=[];
    api=async(path,method,patch)=>{sent.push(patch);return {...tasks[0],seq:tasks[0].seq+1};};`);
  await a.run('runBulkChange({field:"assignee",value:""})');
  await a.run('runBulkChange({field:"field:area",value:""})');
  assert.equal(a.run('sent.length'),0,'a value nobody chose must not write');
  assert.equal(a.run('bulkReady({field:"assignee",value:" "})'),false);
  assert.equal(a.run('bulkReady({field:"field:area",value:""})'),false);
  assert.equal(a.run('bulkReady({field:"add-tags",value:" , "})'),false);
  assert.equal(a.run('bulkReady({field:"unassign"})'),true);
  assert.equal(a.run('bulkReady({field:"clear:area"})'),true);
  a.run('renderFilters()');
  const menu=a.element('#bulk-field').innerHTML;
  assert.match(menu,/value="unassign"[^>]*>Unassign owner/);
  assert.match(menu,/value="clear:area"[^>]*>Clear Area/);
  a.element('#bulk-field').value='field:area'; a.run('renderBulkValue()');
  const control=a.element('#bulk-value-control').innerHTML;
  assert.match(control,/^<select[^>]*><option value="">Choose area…<\/option>/,'the first, default option chooses nothing');
  assert.doesNotMatch(control,/Clear/,'clearing is chosen in the property menu, not defaulted here');
});

test('shortcuts still work while a task checkbox has focus, but not while typing',()=>{
  const a=app(); let searched=false;
  a.element('#search').focus=()=>{searched=true;};
  const checkbox=target({inside:['#tasks']});
  a.run('selectTask("1",true)');
  a.dispatch('keydown',key('Escape',checkbox));
  assert.equal(a.run('selected.size'),0,'Escape clears the selection');
  const selectAll=a.dispatch('keydown',key('a',checkbox,{metaKey:true}));
  assert.equal(a.run('selected.size'),3,'Cmd/Ctrl+A selects the tasks in view');
  assert.equal(selectAll.defaultPrevented,true);
  a.dispatch('keydown',key('/',checkbox));
  assert.equal(searched,true,'/ moves to search');
  a.dispatch('keydown',key('Escape',target({type:'text',inside:['#filters']})));
  assert.equal(a.run('selected.size'),3,'Escape in a text field is left to the field');
});

test('Shift-click after clearing the selection starts a new range',()=>{
  const a=app();
  a.run('selectTask("1",true); clearSelection(); selectTask("3",true,true)');
  assert.equal(a.run('JSON.stringify([...selected])'),'["3"]');
});

test('list rows show who is working on a task',()=>{
  const a=app();
  a.run(`tasks[1].lease={active:true,holder:'agent:codex',node:'cx-1'};
    tasks[2].lease={active:false,holder:'agent:gone',node:'old'}; renderRealTasks()`);
  const html=a.element('#tasks').innerHTML;
  assert.match(html,/agent:codex/);
  assert.doesNotMatch(html,/agent:gone/,'an inactive lease is not a holder');
});

test('the bulk bar says when held tasks will keep their status',()=>{
  const a=app();
  a.run(`tasks[1].lease={active:true,holder:'agent:codex',node:'cx-1'}; selected.add('1'); selected.add('2')`);
  a.element('#bulk-field').value='state';
  assert.match(a.run('bulkNote()'),/1 held .*status/i);
  a.element('#bulk-field').value='priority';
  assert.equal(a.run('bulkNote()'),'','held tasks still take detail changes');
});

test('a batch that changes nothing keeps the previous batch undoable',async()=>{
  const a=app();
  a.run(`selected.add('1'); selected.add('2'); var failing=false;
    api=async(path,method,patch)=>{const t=taskById(path.split('/').pop());
      if(failing)throw Object.assign(new Error('Updated elsewhere'),{kind:'stale'});
      return {...t,...patch,seq:t.seq+1};};`);
  await a.run('runBulkChange({field:"priority",value:"high"})');
  a.run('failing=true');
  await a.run('runBulkChange({field:"priority",value:"low"})');
  assert.equal(a.run('bulkResult.updated.length'),0);
  assert.equal(a.run('bulkUndo.map(item=>item.id).join(",")'),'1,2');
  assert.match(a.element('#bulk-result').innerHTML,/Undo previous batch/);
  a.run('failing=false');
  await a.run('undoBulkChange()');
  assert.equal(a.run('tasks.map(t=>t.priority).join(",")'),'none,none,none');
});

test('a task list read before a save finishes cannot roll the save back',async()=>{
  const a=app();
  // The load-time refresh never settles here (fetch never resolves); start fresh.
  a.run(`refreshing=false; var server=structuredClone(tasks[0]), reads=0, releaseFirst;
    api=(path,method,body)=>{
      if(path==='/api/state'){
        reads++;
        const snapshot={tasks:[structuredClone(server),tasks[1],tasks[2]],fields:{fields:[]},changed:[],store:'/work/.hippotask'};
        return reads===1 ? new Promise(r=>{releaseFirst=()=>r(snapshot);}) : Promise.resolve(snapshot);
      }
      const {base,...patch}=body; server={...server,...patch,seq:server.seq+1};
      return Promise.resolve(structuredClone(server));};
    selected.add('1');`);
  const reading=a.run('realRefresh()');
  await a.run('runBulkChange({field:"priority",value:"high"})');
  a.run('releaseFirst()');
  await reading;
  assert.equal(a.run('taskById("1").priority'),'high');
  assert.equal(a.run('reads'),2,'the outdated snapshot is discarded and read again');
});

test('new tasks start from the project\'s description template, ready to write',()=>{
  const a=app();
  a.run(`taskFormat={guide:null,template:"## Why\\n\\n## Done when\\n- [ ]\\n",required_fields:[],required_sections:["Done when"]};`);
  a.run('createTask()');
  assert.equal(a.run('drafts.get("new").values.body'),'## Why\n\n## Done when\n- [ ]\n');
  assert.equal(a.run('drafts.get("new").descriptionMode'),'write');
  a.run('discard("new"); taskFormat={guide:null,template:null,required_fields:[],required_sections:[]}; createTask()');
  assert.equal(a.run('drafts.get("new").values.body'),'','no template: a blank description');
});

test('the task list read carries the format along',async()=>{
  const a=app();
  a.run(`refreshing=false; api=()=>Promise.resolve({tasks,fields:{fields:[]},changed:[],store:'/work/.hippotask',
    format:{guide:"Verbs first.",template:"## Why\\n",required_fields:[],required_sections:[]}});`);
  await a.run('realRefresh()');
  assert.equal(a.run('taskFormat.template'),'## Why\n');
});

test('required fields the draft leaves unset are named',()=>{
  const a=app();
  a.run(`fields=[{field:"project",display_name:"Project",values:["dashboard"]},{field:"team",display_name:"Team",values:null}];
    taskFormat={guide:null,template:null,required_fields:["project"],required_sections:[]};`);
  assert.equal(a.run('JSON.stringify(missingRequired(newDraft(tasks[0])))'),'["Project"]');
  assert.equal(a.run('JSON.stringify(missingRequired(newDraft({...tasks[0],fields:{project:"dashboard"}})))'),'[]');
  assert.equal(a.run('JSON.stringify(missingRequired(newDraft({...tasks[0],state:"done"})))'),'[]','closed tasks are not checked');
  assert.equal(a.run('isRequired(fields[0])'),true);
  assert.equal(a.run('isRequired(fields[1])'),false);
});

test('format gaps stay out of the warning banner and read as plain words',()=>{
  const a=app();
  a.run(`var result={warnings:["CLI text for project","CLI text for done when","the ledger has a torn line"],
    format_gaps:[{kind:"field",name:"project",label:"Project",message:"CLI text for project"},
                 {kind:"section",name:"Done when",label:"Done when",message:"CLI text for done when"}]};`);
  assert.equal(a.run('JSON.stringify(visibleWarnings(result))'),'["the ledger has a torn line"]','other warnings still show');
  assert.equal(a.run('JSON.stringify(visibleWarnings({warnings:["x"]}))'),'["x"]','no gaps: nothing hidden');
  assert.equal(a.run('gapText(result.format_gaps[0])'),'Project');
  assert.equal(a.run('gapText(result.format_gaps[1])'),'text under “Done when”');
  a.run('formatGaps.set("1", result.format_gaps)');
  assert.match(a.run('gapsNotice("1")'),/Project · text under “Done when”/);
  assert.doesNotMatch(a.run('gapsNotice("1")'),/hippo-task/,'no CLI commands for people');
  assert.equal(a.run('gapsNotice("2")'),'','a task without gaps shows nothing');
});
