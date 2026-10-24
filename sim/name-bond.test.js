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

test('the bond does not change the one-off loss; it stays locked while the name is squatted', () => {
  assert.equal(squatCost(1, 0, 0).once, squatCost(1, 0.25, 0).once);
  assert.ok(squatCost(1, 0.25, 1).total > squatCost(1, 0, 1).total);
  const c = squatCapital(1000, 1, 0.25);
  assert.ok(Math.abs(c.locked - 1000 * (0.25 + COSTS.recordRent)) < 1e-9);
});

test('an honest creator loses only the launch rent and a little yield', () => {
  const c = creatorCost(0.25, 1);
  assert.ok(c.upfront > 0.25 && c.upfront < 0.26);
  assert.ok(c.lost < 0.008);
});
