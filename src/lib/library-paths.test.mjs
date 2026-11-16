import assert from 'node:assert/strict';
import test from 'node:test';
import { isLibraryFolderCovered } from './library-paths.ts';

test('library folder coverage respects Windows ancestors, casing and path boundaries', () => {
  assert.equal(isLibraryFolderCovered('C:\\Music\\Album', ['c:/music/']), true);
  assert.equal(isLibraryFolderCovered('C:\\Music', ['C:\\Music']), true);
  assert.equal(isLibraryFolderCovered('C:/Musical/Album', ['C:/Music']), false);
  assert.equal(isLibraryFolderCovered('D:/Music', ['C:/Music']), false);
  assert.equal(isLibraryFolderCovered('C:/Music/音楽', ['C:/']), true);
});

test('UNC and Unix folders retain their relevant case and boundary semantics', () => {
  assert.equal(isLibraryFolderCovered('\\\\Server\\Audio\\Album', ['//server/audio/']), true);
  assert.equal(isLibraryFolderCovered('//server/audiobooks', ['//server/audio']), false);
  assert.equal(isLibraryFolderCovered('/music/Album', ['/music']), true);
  assert.equal(isLibraryFolderCovered('/Music/Album', ['/music']), false);
  assert.equal(isLibraryFolderCovered('/music', ['/']), true);
  assert.equal(isLibraryFolderCovered('/musical', ['/music']), false);
});

test('folders removed from inside a library folder are no longer covered', () => {
  assert.equal(isLibraryFolderCovered('C:/Music/Live', ['C:/Music'], ['c:\\music\\live']), false);
  assert.equal(isLibraryFolderCovered('C:/Music/Live/Disc 1', ['C:/Music'], ['C:/Music/Live']), false);
  assert.equal(isLibraryFolderCovered('C:/Music/Lively', ['C:/Music'], ['C:/Music/Live']), true);
});
