import assert from 'node:assert/strict';
import { test } from 'node:test';
import { normalizeSearch, rankSearch } from './power-search.ts';
const items = [
  { id:'next',label:'Nächster Titel',detail:'Wiedergabe',keywords:'weiter next',kind:'action' },
  { id:'repeat',label:'Nächsten Titel spielen',detail:'Wiederholung',kind:'action' },
  { id:'song',label:'Nächster Morgen',detail:'Künstler',kind:'track' },
];
test('search separates songs and actions and normalizes accents',()=>{ assert.equal(normalizeSearch('NÄCHSTER!'),'nachster');assert.deepEqual(rankSearch(items,'nächster').map(x=>x.item.id),['song']);assert.ok(rankSearch(items,'>nächster').every(x=>x.item.kind==='action')); });
test('minor typing errors find actions without matching unrelated text',()=>{assert.equal(rankSearch(items,'>nachstr')[0].item.id,'next');assert.equal(rankSearch(items,'>xxxxxxxx').length,0);});
test('learning reorders comparable results but cannot outrank exact matches',()=>{const now=1000;assert.equal(rankSearch(items,'>nächsten titel spielen',{'nachsten titel spielen|next':{count:1000,usedAt:now}},now)[0].item.id,'repeat');const results=rankSearch(items,'>titel',{'titel|repeat':{count:10,usedAt:now}},now);assert.equal(results[0].item.id,'repeat');});
test('disabled actions are not executable search results',()=>{assert.equal(rankSearch([{...items[0],disabled:true}],'>next').length,0);});
