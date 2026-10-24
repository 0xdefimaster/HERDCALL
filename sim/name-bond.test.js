import { test } from 'node:test';
import assert from 'node:assert/strict';
import { COSTS, creatorCost, squatCapital, squatCost, squatLoss } from './name-bond.js';

test('a minimum Lobby makes squatting cost real SOL, growing faster than the minimum', () => {
  const l1 = squatLoss(1).loss;
  const l2 = squatLoss(2).loss;
  assert.ok(l1 > 0.03 && l1 < 0.06);
  assert.ok(l2 > 3 * l1, 'the Stampede surplus grows with the square of the Lobby');
  // Almost all of it is the surplus left in the curve.
  assert.ok(squatLoss(2).surplus > 0.8 * l2);
});

test('the bond only raises the capital needed at once, not what is lost', () => {
  const a = squatCapital(1000, 1, 0);
  const b = squatCapital(1000, 1, 0.25);
  assert.equal(a.lost, b.lost);
  assert.ok(Math.abs(b.peak - a.peak - 250) < 1e-6);
  assert.ok(Math.abs(squatCost(1) - (squatLoss(1).loss + COSTS.sunkPerLaunch + COSTS.txPerSquat)) < 1e-12);
});

test('an honest creator gets the bond back within the Lobby and loses only the launch rent', () => {
  const c = creatorCost(0.25, 1);
  assert.ok(c.upfront > 0.25 && c.upfront < 0.26);
  assert.ok(c.lost - COSTS.sunkPerLaunch < 0.00001);
});
