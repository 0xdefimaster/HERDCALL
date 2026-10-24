import { test } from 'node:test';
import assert from 'node:assert/strict';
import { COSTS, creatorCost, grabCapital, holdCost } from './name-bond.js';

test('the bond scales the capital of a mass grab, not the cost of holding one name', () => {
  assert.ok(grabCapital(1000, 1) > 100 * grabCapital(1000, 0));
  const free = holdCost(7, 0).total;
  const bonded = holdCost(7, 1).total;
  // One week with a 1 SOL bond costs barely more than with none: the bond comes back.
  assert.ok(bonded - free < 0.002);
  assert.ok(Math.abs(free - (COSTS.sunkPerLaunch + COSTS.txPerCycle)) < 1e-4);
});

test('holding a name for a year costs one launch per curve lifetime', () => {
  const y = holdCost(365, 0.5);
  assert.equal(y.cycles, 53);
  assert.ok(y.total > 53 * COSTS.sunkPerLaunch);
});

test('an honest creator loses only the launch rent and a few days of yield', () => {
  const c = creatorCost(1, 7);
  assert.ok(c.upfront > 1);
  assert.ok(c.lost < 0.01);
});
