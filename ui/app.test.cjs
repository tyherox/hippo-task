const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

function app() {
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
    document:{querySelector:element,querySelectorAll:()=>[],addEventListener(){},activeElement:null},
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
    renderTasks=()=>{}; renderEditor=()=>{}; renderSelection=()=>{};
    refresh=async()=>{}; toast=()=>{};
  `,context);
  return {run:code=>vm.runInContext(code,context), element, context};
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
