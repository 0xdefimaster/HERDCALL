import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bestSandwich, reinvest, safeShare } from './lp-reinvest.js';

test('reinvest adds at the pool ratio and swaps about half', () => {
  const pool = { x: 100, y: 1000 };
  const r = reinvest(pool, 2, 0.01);
  assert.ok(r.swapped > 0.95 && r.swapped < 1.05);
  assert.ok(Math.abs(r.addedSol / r.addedTokens - r.pool.x / r.pool.y) < 1e-6);
});

test('small reinvests cannot be sandwiched profitably, large ones can', () => {
  const pool = { x: 120, y: 160_000_000 };
  assert.equal(bestSandwich(pool, 1, 0.01).profit, 0);
  assert.ok(bestSandwich(pool, 20, 0.01).profit > 0);
});

test('the safe share grows with the pool fee', () => {
  assert.ok(safeShare(0.02) > safeShare(0.01));
  assert.ok(safeShare(0.01) > safeShare(0.0025));
});
