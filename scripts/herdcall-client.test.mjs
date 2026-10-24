import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { canonicalName, canonicalTicker, nameHash } from './herdcall-client.mjs';

// Same vectors as programs/herdcall-tests/tests/names.rs, from the program's own name_seed.
const vectors = JSON.parse(readFileSync(new URL('../programs/idl/name-vectors.json', import.meta.url), 'utf8'));

test('name hashes match the program', () => {
  for (const { name, sha256 } of vectors) assert.equal(Buffer.from(nameHash(name)).toString('hex'), sha256, name);
});

test('canonical forms', () => {
  assert.equal(canonicalTicker(' pepe '), 'PEPE');
  assert.equal(canonicalTicker('PE PE'), null);
  assert.equal(canonicalTicker('ABCDEFGHIJK'), null);
  assert.equal(canonicalName('  PEPE Coin '), 'pepe coin');
  assert.equal(canonicalName('   '), null);
  assert.equal(canonicalName('a\nb'), null);
  assert.equal(canonicalName('x'.repeat(33)), null);
});
