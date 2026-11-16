import { readFileSync, readdirSync } from 'node:fs';
import assert from 'node:assert/strict';

function flatten(value, prefix = '', result = {}) {
  for (const [key, text] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof text === 'object' && text !== null) flatten(text, path, result);
    else { assert.equal(typeof text, 'string', path); assert.ok(text.length, path); result[path] = text; }
  }
  return result;
}
const localeDirectory = new URL('../src/i18n/locales/', import.meta.url);
const read = language => {
  const result = {};
  for (const name of readdirSync(localeDirectory).filter(name => name === `${language}.json` || name.endsWith(`.${language}.json`))) {
    const entries = flatten(JSON.parse(readFileSync(new URL(name, localeDirectory), 'utf8')));
    for (const [key, value] of Object.entries(entries)) {
      assert.ok(!(key in result), `Duplicate translation key: ${key}`);
      result[key] = value;
    }
  }
  return result;
};
const de = read('de');
const en = read('en');
assert.deepEqual(Object.keys(de).sort(), Object.keys(en).sort(), 'Translation keys must match');
const placeholders = value => [...value.matchAll(/\{\{\s*([^},]+)(?:,[^}]*)?\s*\}\}/g)].map(match => match[1].trim()).sort();
for (const key of Object.keys(de)) assert.deepEqual(placeholders(de[key]), placeholders(en[key]), `Placeholders differ: ${key}`);
for (const key of ['file', 'openFiles', 'openFolder', 'quit', 'playPause', 'previous', 'next', 'stop', 'playback', 'show']) assert.ok(de[`native.${key}`], `Missing native menu: ${key}`);
console.log(`DE/EN: ${Object.keys(de).length} matching translation keys, placeholders and native menus verified.`);

