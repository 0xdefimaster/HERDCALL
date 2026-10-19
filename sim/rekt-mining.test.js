// Checks for document/REKT_MINING.md. Run: npm run test:sim
import test from 'node:test';
import assert from 'node:assert/strict';
import { NATIVE } from './stampede.js';
import {
  HONEST_MIX,
  HerdPool,
  POLICY,
  RektTreasury,
  bestFarm,
  eligibleRevenue,
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
  runPurchase,
  sandwich,
  scoreSeason,
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
    assert.ok(r.farmDh <= x / 0.3 + 1e-9, 'rebate never above the fees the farmer paid');
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

test('eligible revenue: only fees collected this season, never reserves or price gains', () => {
  const base = { protocolFeesCollected: 450, reversed: 20 };
  const noisy = { ...base, carriedSol: 300, unclaimedHerdCost: 50, herdPriceGain: 999, unspentSol: 40 };
  assert.equal(eligibleRevenue(base), 430);
  assert.equal(eligibleRevenue(noisy), 430);
  assert.equal(eligibleRevenue({ protocolFeesCollected: 5, reversed: 9 }), 0);
});

test('scoring: closing season counts, the hold clock crosses seasons, each close is scored once', () => {
  const positions = [
    { id: 'a', owner: 'u1', openedAt: 25, closedAt: 33, basis: 1, loss: true }, // opened in s1, closed in s2, 8 days
    { id: 'b', owner: 'u1', openedAt: 29, closedAt: null, basis: 1, loss: true }, // still open
    { id: 'c', owner: 'u2', openedAt: 31, closedAt: 34, basis: 1, loss: true }, // 3 days, no gate
  ];
  const scored = new Set();
  const s1 = scoreSeason(positions, { start: 0, end: 30, scored });
  assert.equal(s1.total, 0, 'nothing closed in season 1');
  const s2 = scoreSeason(positions, { start: 30, end: 60, scored });
  assert.equal(s2.byOwner.get('u1'), 1.875, 'held 8 days across the boundary: hold 1.25 × rekt 1.5');
  assert.equal(s2.byOwner.get('u2'), 1, '3 days: no rekt multiplier');
  const again = scoreSeason(positions, { start: 30, end: 60, scored });
  assert.equal(again.total, 0, 'the same close is never scored twice');
  positions[1].closedAt = 70;
  const s3 = scoreSeason(positions, { start: 60, end: 90, scored });
  assert.equal(s3.byOwner.get('u1'), 1.875, 'scored only in the season it closes');
});

test('treasury: a season never distributes more than its cap, with or without carry', () => {
  const next = rng(9);
  const tr = new RektTreasury({ opsFloor: 50, claimWindow: 2 });
  for (let t = 1; t <= 40; t += 1) {
    const revenue = next() * 2000;
    const points = new Map([['a', 1 + next()], ['b', 1 + next()], ['c', next() < 0.5 ? 0 : 1]]);
    const s = tr.close(t, { revenue, points });
    assert.ok(s.allocation <= POLICY.alpha * revenue + 1e-9);
    const spend = s.solToBuy * next();
    tr.purchase(t, spend, spend * 100);
    tr.endPurchase(t, t);
    for (const owner of ['a', 'b']) if (next() < 0.6) tr.claim(t, owner, t);
    if (t > 2) tr.expire(t - 2, t);
    assert.ok(Math.abs(tr.distributedCost(t) - s.allocation) < 1e-6, 'carry + spent + owed = allocation');
  }
});

test('treasury: unspent SOL stays with its season, the next allocation ignores it', () => {
  const points = new Map([['a', 1]]);
  const withLeftover = new RektTreasury();
  withLeftover.close(1, { revenue: 1000, points });
  withLeftover.endPurchase(1, 1); // nothing bought: all 250 owed in SOL to season 1
  const clean = new RektTreasury();
  clean.close(1, { revenue: 1000, points });
  clean.purchase(1, 250, 25_000);
  clean.endPurchase(1, 1);
  assert.equal(withLeftover.get(1).solOwed, 250);
  assert.equal(withLeftover.close(2, { revenue: 600, points }).allocation, clean.close(2, { revenue: 600, points }).allocation);
  assert.deepEqual(withLeftover.claim(1, 'a', 2), { tokens: 0, sol: 250 });
});

test('treasury: claims are once, inside the window; expired $HERD becomes carry used inside a later cap', () => {
  const tr = new RektTreasury({ claimWindow: 2 });
  tr.close(1, { revenue: 1000, points: new Map([['a', 1], ['b', 3]]) });
  tr.purchase(1, 250, 1000);
  tr.endPurchase(1, 1);
  assert.deepEqual(tr.claim(1, 'b', 2), { tokens: 750, sol: 0 });
  assert.throws(() => tr.claim(1, 'b', 2), /already claimed/);
  assert.throws(() => tr.claim(1, 'a', 3), /window closed/);
  assert.throws(() => tr.expire(1, 2), /still open/);
  assert.deepEqual(tr.expire(1, 3), { tokens: 250, cost: 62.5 });
  const s2 = tr.close(2, { revenue: 400, points: new Map([['a', 1]]) });
  assert.equal(s2.allocation, 100);
  assert.equal(s2.carryUsed, 62.5, 'carry counted at cost inside the cap');
  assert.equal(s2.solToBuy, 37.5, 'new purchases only for the rest of the cap');
  assert.equal(tr.released, 62.5, 'the SOL the carry replaces goes back to the protocol');
});

const freshPool = (feeBps = 25) => new HerdPool({ sol: 1000, herd: 10_000_000, feeBps });

test('purchase: splitting does not lower the total cost without arbitrage', () => {
  for (const ratio of [0.01, 0.05, 0.1]) {
    const one = freshPool().impact(ratio * 1000);
    const split = runPurchase(freshPool(), { budget: ratio * 1000, chunks: 28, limit: 1 });
    assert.ok(Math.abs(split.totalPremium - one) / one < 0.01, `ratio ${ratio}: ${split.totalPremium} vs ${one}`);
    const arb = runPurchase(freshPool(), { budget: ratio * 1000, chunks: 28, limit: 1, rho: 1 });
    assert.ok(arb.totalPremium < split.totalPremium);
  }
});

test('purchase: no executed trade exceeds the per-trade limit, even when every chunk is front-run', () => {
  const next = rng(10);
  for (let i = 0; i < 200; i += 1) {
    const pool = freshPool(Math.floor(next() * 50));
    const budget = next() * 200;
    const r = runPurchase(pool, { budget, chunks: 1 + Math.floor(next() * 40), attacker: next() < 0.7, rho: next() });
    assert.ok(r.maxImpact <= 0.005 + 1e-12);
    assert.ok(Math.abs(r.spent + r.leftover - budget) < 1e-9);
    if (r.halted) assert.ok(r.leftover > 0);
  }
});

test('purchase: stops when the estimate exceeds the limit or liquidity is withdrawn, SOL stays unspent', () => {
  const big = runPurchase(freshPool(), { budget: 100, chunks: 10 }); // 1% per chunk
  assert.equal(big.spent, 0);
  assert.equal(big.halted.reason, 'estimate above limit');
  const pulled = runPurchase(freshPool(), {
    budget: 100,
    chunks: 28,
    onChunk: (pool, i) => i === 10 && pool.withdraw(0.5),
  });
  assert.equal(pulled.halted.at, 10);
  assert.ok(Math.abs(pulled.leftover - (100 * 18) / 28) < 1e-9);
});
