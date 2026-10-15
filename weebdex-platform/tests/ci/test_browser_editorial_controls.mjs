#!/usr/bin/env node
// Reconstructed strict-locator seam. No browser, service or success response.
import assert from 'node:assert/strict';
import test from 'node:test';
import { contributorSearchTrigger } from '../../scripts/browser-editorial-controls.mjs';
function fixture(family, count=1) {
  const events=[],scope={ getByText(name,options){assert.equal(name,'Search...');assert.deepEqual(options,{exact:true});return{async click(){assert.equal(count,1,'strict unique contributor field');events.push(family);}};} };
  return {events,page:{getByText(){throw Error('Global header text must never be selected');},locator(selector){assert.equal(selector,`label[for="${family}"]`);return{locator(parent){assert.equal(parent,'..');return scope;}};}}};
}
test('author and artist searches stay in their labeled fields despite identical header text',async()=>{for(const family of ['authors','artists']){const value=fixture(family);await contributorSearchTrigger(value.page,family).click();assert.deepEqual(value.events,[family]);}});
for(const family of ['authors','artists'])for(const count of [0,2])test(`${family} rejects absent or duplicate trigger:${count}`,async()=>{const value=fixture(family,count);await assert.rejects(contributorSearchTrigger(value.page,family).click());});
test('unknown fields cannot expand selector scope',()=>{assert.throws(()=>contributorSearchTrigger({},'header'));});
