// Checks for document/REKT_MINING.md. Run: npm run test:sim
import test from 'node:test';
import assert from 'node:assert/strict';
import { NATIVE } from './stampede.js';
import {
  HONEST_MIX,
  POLICY,
  bestFarm,
  buyback,
  cappedSafetyMargin,
  fixedPoolOptimum,
  freshCurve,
  honestFlow,
  honestMultiplier,
  multiplier,
  pairTrip,
  policyPool,
  rng,
  roundTrip,
  sandwich,
  seasonPayoff,
  strategyMultiplier,
} from './rekt-mining.js';

const PROPOSAL = { mRekt: 1.5, mHold: 1.25, gate: true, gateDays: 7, cap: 2 };
const sizeFrom = (next) => BigInt(Math.floor(next() * 20 * Number(NATIVE))) + NATIVE / 100n;

test('round trip on the curve: always a loss, never cheaper than fees minus kickbacks', () => {
  const next = rng(1);
  for (let i = 0; i < 300; i += 1) {
    const curve = freshCurve();
    honestFlow(curve, next, { trades: 30, drift: 0.7 });
    const opts = { creator: next() < 0.5, iface: next() < 0.5 };
    const r = roundTrip(curve, sizeFrom(next), opts);
    assert.ok(r.positions[0].loss, 'a round trip closes at a loss');
    const feePaid = (r.basis * 100n) / 45n; // protocol fee is 45 of the 100 bps
    assert.ok(r.netCost + r.kickback >= feePaid - 2n, 'the farmer pays at least the full fee');
  }
});

test('pair trip: the cluster never profits, and the second wallet shows a manufactured loss', () => {
  const next = rng(2);
  for (let i = 0; i < 300; i += 1) {
    const curve = freshCurve();
    honestFlow(curve, next, { trades: 30, drift: 0.7 });
    const r = pairTrip(curve, sizeFrom(next));
    assert.ok(r.netCost > 0n);
    assert.equal(r.positions[1].loss, true);
  }
});

test('hold gate: positions held 0 days never earn the rekt multiplier', () => {
  const curve = freshCurve();
  const r1 = roundTrip(curve, 5n * NATIVE);
  const r2 = pairTrip(curve, 5n * NATIVE);
  assert.equal(strategyMultiplier(r1.positions, PROPOSAL), 1);
  assert.equal(strategyMultiplier(r2.positions, PROPOSAL), 1);
  // Without the gate the same round trip earns the full rekt multiplier for free.
  assert.equal(strategyMultiplier(r1.positions, { ...PROPOSAL, gate: false }), 1.5);
});

test('multipliers never exceed the cap', () => {
  const next = rng(3);
  for (let i = 0; i < 2000; i += 1) {
    const design = { mRekt: 1 + next() * 4, mHold: 1 + next() * 4, gate: next() < 0.5, cap: 1 + next() * 3 };
    const m = multiplier({ heldDays: Math.floor(next() * 30), loss: next() < 0.5 }, design);
    assert.ok(m >= 1 && m <= design.cap);
  }
  assert.ok(honestMultiplier(PROPOSAL) >= 1 && honestMultiplier(PROPOSAL) <= PROPOSAL.cap);
});

function randomSeason(next) {
  const mBar = 1 + next() * 0.8;
  return {
    H: 100 + next() * 1000,
    mBar,
    mF: 1 + next() * 2,
    cF: 0.6 + next() * 2,
    alpha: next(),
    budgetValue: next() < 0.3 ? Infinity : next() * 5000,
    dh: { ellH: 0.05 + next() * 0.5, farmerEligible: next() },
  };
}

test('capped pool: when the safety margin is >= 0, no farm size is profitable', () => {
  const next = rng(4);
  let checked = 0;
  for (let i = 0; i < 3000 && checked < 300; i += 1) {
    const p = { pool: 'capped', ...randomSeason(next) };
    if (cappedSafetyMargin(p) < 0) continue;
    checked += 1;
    const best = bestFarm(p, { steps: 600 });
    assert.ok(best.profit <= 1e-9 * p.H, `profit ${best.profit} with margin ${cappedSafetyMargin(p)}`);
  }
  assert.equal(checked, 300);
});

test('capped pool: the condition is tight — a negative margin admits a profitable farm', () => {
  const next = rng(5);
  let checked = 0;
  for (let i = 0; i < 5000 && checked < 200; i += 1) {
    const p = { pool: 'capped', ...randomSeason(next), budgetValue: Infinity };
    // The bound is reached when the farmer out-multiplies and out-qualifies honest users.
    if (p.mF < p.mBar || p.dh.farmerEligible < p.dh.ellH || cappedSafetyMargin(p) > -0.05) continue;
    checked += 1;
    assert.ok(bestFarm(p, { steps: 600 }).profit > 0);
  }
  assert.equal(checked, 200);
});

test('fixed pool: farming starts exactly at V·mF = cF·mBar·H and the grid matches the closed form', () => {
  const p = { pool: 'fixed', H: 450, mBar: 1.2, mF: 1.5, cF: 1.333 };
  const threshold = (p.cF * p.mBar * p.H) / p.mF;
  assert.equal(fixedPoolOptimum({ ...p, budgetValue: threshold * 0.999 }).x, 0);
  assert.ok(fixedPoolOptimum({ ...p, budgetValue: threshold * 1.01 }).x > 0);
  for (const V of [600, 1000, 5000]) {
    const closed = fixedPoolOptimum({ ...p, budgetValue: V });
    const grid = bestFarm({ ...p, budgetValue: V });
    assert.ok(Math.abs(grid.profit - closed.profit) / closed.profit < 0.01);
  }
});

test('separate accounting: season rewards sum to the pool, Diamond Hooves never exceeds fees paid', () => {
  const next = rng(6);
  for (let i = 0; i < 2000; i += 1) {
    const p = { pool: next() < 0.5 ? 'capped' : 'fixed', ...randomSeason(next) };
    if (p.budgetValue === Infinity) p.budgetValue = 1000;
    const x = next() * 5 * p.H;
    const r = seasonPayoff(x, p);
    assert.ok(Math.abs(r.farmReward + r.honestReward - r.value) < 1e-6 * (1 + r.value));
    assert.ok(r.farmDh <= x / 0.45 + 1e-9, 'rebate never above the fees the farmer paid');
    if (p.pool === 'capped') assert.ok(r.value <= p.alpha * (p.H + x) + 1e-9);
  }
});

test('sandwich pays without any reward: MEV is a separate problem from Rekt Mining', () => {
  const r = sandwich(freshCurve(), 5n * NATIVE);
  assert.ok(r, 'a front-run fits inside 1% slippage');
  assert.ok(r.netCost < 0n, 'the sandwich is profitable on its own');
  assert.ok(r.honestTokens >= r.minOut);
});

test('honest mix sums to 1', () => {
  const total = Object.values(HONEST_MIX).reduce((a, b) => a + b, 0);
  assert.ok(Math.abs(total - 1) < 1e-12);
});

test('policy pool: funds Diamond Hooves and the ops floor first, never above alpha × revenue', () => {
  const next = rng(7);
  for (let i = 0; i < 2000; i += 1) {
    const revenue = next() * 10_000;
    const opsFloor = next() * 2000;
    const alpha = next() * POLICY.alpha;
    const pool = policyPool(revenue, { alpha, opsFloor });
    assert.ok(pool >= 0);
    assert.ok(pool <= alpha * revenue + 1e-9);
    assert.ok(pool + revenue / POLICY.dhDivisor + opsFloor <= revenue + 1e-9 || pool === 0);
  }
});

test('policy pool: with the margin holding, no farm pays at any revenue level or ops floor', () => {
  const next = rng(8);
  let checked = 0;
  for (let i = 0; i < 3000 && checked < 200; i += 1) {
    const p = { pool: 'policy', ...randomSeason(next), alpha: next() * POLICY.alpha, opsFloor: next() * 2000 };
    if (cappedSafetyMargin(p) < 0) continue;
    checked += 1;
    assert.ok(bestFarm(p, { steps: 600 }).profit <= 1e-9 * p.H);
  }
  assert.equal(checked, 200);
});

test('buyback: premium per chunk is chunk / liquidity on a constant-product pool', () => {
  for (const [value, liquidity, chunks] of [[100, 1000, 1], [100, 1000, 20], [500, 20_000, 7]]) {
    const r = buyback({ value, liquidity, price: 0.01, chunks });
    assert.ok(Math.abs(r.premium - value / chunks / liquidity) < 1e-9);
  }
  assert.ok(buyback({ value: 100, liquidity: 1000, price: 1, chunks: 10 }).tokens > buyback({ value: 100, liquidity: 1000, price: 1 }).tokens);
});
