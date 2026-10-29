import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
const read = path => fs.readFileSync(new URL('../../platform-integrations/' + path, import.meta.url), 'utf8');
const source = read('local-library/IdentityGate.svelte');
function gate() {
  let finishes = 0, destroy;
  const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
  const context = vm.createContext({Error, $props: () => ({onfinish: () => finishes++}), $state: x => x, onDestroy: fn => destroy = fn});
  const api = vm.runInContext('(function(){' + stripTypeScriptTypes(script, {mode:'transform'}) + ';return {setOpen,finish,restoreTaskFocus,bindFocus(button,anchor){trigger=button;returnAnchor=anchor},current:()=>guard(generation)};})()', context);
  return {api, finishes:()=>finishes, destroy:()=>destroy()};
}
test('one shared setup flow resumes explicitly, never on connection or dismiss', () => {
  const f = gate(); f.api.setOpen(true); const current = f.api.current(); current();
  assert.equal(f.finishes(),0); f.api.setOpen(false); assert.throws(current,/closed/); assert.equal(f.finishes(),0);
  f.api.setOpen(true); const next=f.api.current(); f.api.finish(); assert.equal(f.finishes(),1); assert.throws(next,/closed/);
});
test('reopened and disposed gates cannot finish stale setup work', () => {
  const f=gate(); f.api.setOpen(true); const old=f.api.current(); f.api.setOpen(false); f.api.setOpen(true);
  assert.throws(old,/closed/);const current=f.api.current();current();f.destroy();assert.throws(current,/closed/);
});
test('gate owns dialog independently of trigger visibility and has no redirect surface',()=>{
 assert.match(source,/\{#if show\}[\s\S]*\{\/if\}\s*<Dialog.Root/);
 assert.match(source,/<IdentitySetup[^>]+onfinish=\{finish\}/);
 assert.doesNotMatch(source,/window.location|goto\(|returnTo|admitAccount|mutate\(/);
});
test('upload stays in task and retries only account lookup on finish',()=>{
 const spec=JSON.parse(read('public-api/upload-anchors.json'));const gate=spec.patches.find(p=>p.label==='missing signer is an explicit setup state').after;
 assert.match(gate,/<IdentityGate[^>]+show=\{!\$signerIdentity.owner\}/);assert.ok(gate.indexOf('<IdentityGate')<gate.indexOf('{#if !$signerIdentity.owner}'));
 assert.match(gate,/onfinish=\{\(\) => setupRevision\+\+\}/);assert.doesNotMatch(gate,/href="\/settings"/);
});
test('signup keeps ordinary reading and verification separate from advanced signing setup',()=>{
 const source=read('frontend-behavior/AccountOnboarding.svelte');assert.match(source,/Verify email<\/a>/);assert.match(source,/Continue reading<\/Button>/);assert.doesNotMatch(source,/<IdentitySetup/);
});

// Actual DOM focus targets with the production handler. Modal stacking, Escape
// and Tab trapping remain full-browser checks; no Bits UI behavior is mocked as proof.
test('close restores task focus when connection removes its trigger', async t => {
 const {createRequire}=await import('node:module');
 const require=createRequire(import.meta.url);
 const modules=process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES;
 if(!modules) { t.skip('Set WEEBDEX_BROWSER_DOM_NODE_MODULES for DOM focus check'); return; }
 const {JSDOM}=require(modules+'/jsdom');
 const dom=new JSDOM('<section role="dialog"><span tabindex="-1">Continue with your task</span><button>Set up public identity</button><input aria-label="Task draft" value="keep me"></section>');
 try {
  const button=dom.window.document.querySelector('button'),anchor=dom.window.document.querySelector('span');
  const f=gate();f.api.bindFocus(button,anchor);f.api.setOpen(true);
  let event=new dom.window.Event('close',{cancelable:true});f.api.restoreTaskFocus(event);
  assert.equal(event.defaultPrevented,true);assert.equal(dom.window.document.activeElement,button);
  button.remove();event=new dom.window.Event('close',{cancelable:true});f.api.restoreTaskFocus(event);
  assert.equal(event.defaultPrevented,true);assert.equal(dom.window.document.activeElement,anchor);
  assert.equal(dom.window.document.querySelector('input').value,'keep me');assert.equal(f.finishes(),0);
  f.destroy();dom.window.document.querySelector('input').focus();
  event=new dom.window.Event('close',{cancelable:true});f.api.restoreTaskFocus(event);
  assert.equal(event.defaultPrevented,false);assert.equal(dom.window.document.activeElement.tagName,'INPUT');
 } finally {dom.window.close();}
});
