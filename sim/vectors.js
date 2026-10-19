// Exports test vectors from the JS reference model for the Rust core (programs/herdcall-core/tests).
// Run: npm run vectors   (writes programs/herdcall-core/tests/vectors.json)
import { writeFileSync } from 'node:fs';
import { EXAMPLE, NATIVE, TOKEN, settleMath } from './stampede.js';
import { CURVE_EXAMPLE, CreatorLock, marketAfterLobby, splitFee, splitLpFees } from './curve.js';

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const next = rng(2026);
const randBig = (lo, hi) => lo + (BigInt(Math.floor(next() * 2 ** 52)) * (hi - lo)) / 2n ** 52n;
const str = (o) => JSON.parse(JSON.stringify(o, (_, v) => (typeof v === 'bigint' ? v.toString() : v)));

const params = {
  v0: EXAMPLE.v0,
  t0: EXAMPLE.t0,
  totalSupply: CURVE_EXAMPLE.totalSupply,
  curveSupply: CURVE_EXAMPLE.curveSupply,
  lpSupply: CURVE_EXAMPLE.lpSupply,
  feeBps: EXAMPLE.feeBps,
  minDeposit: EXAMPLE.minDeposit,
  minTotal: EXAMPLE.minTotal,
  migrationFee: CURVE_EXAMPLE.migrationFee,
};

// Stampede settlement and pro-rata claims.
const settle = [];
for (const total of [0n, 1n, EXAMPLE.minTotal - 1n, EXAMPLE.minTotal, NATIVE, 50n * NATIVE, 186n * NATIVE, 10_000n * NATIVE]) {
  settle.push({ total, ...settleMath(EXAMPLE, total) });
}
for (let i = 0; i < 200; i += 1) {
  const total = randBig(EXAMPLE.minTotal, 5_000n * NATIVE);
  settle.push({ total, ...settleMath(EXAMPLE, total) });
}
const claims = [];
for (let i = 0; i < 50; i += 1) {
  const deposits = Array.from({ length: 1 + Math.floor(next() * 30) }, () => randBig(EXAMPLE.minDeposit, 50n * NATIVE));
  const total = deposits.reduce((a, b) => a + b, 0n);
  const r = settleMath(EXAMPLE, total);
  claims.push({ total, x: r.X, deposits, tokens: deposits.map((d) => (d * r.X) / total) });
}

// Fee splits and the creator lock.
const fees = [];
const lpFees = [];
for (let i = 0; i < 300; i += 1) {
  const fee = randBig(0n, 10n ** 13n);
  const lockRatioBps = next() < 0.3 ? 10_000n : randBig(0n, 10_000n);
  for (const kind of ['bonded', 'unbonded']) {
    const hasInterface = next() < 0.7;
    fees.push({ fee, kind, hasInterface, lockRatioBps, out: splitFee(fee, kind, { hasInterface, lockRatioBps }) });
    lpFees.push({ fee, kind, lockRatioBps, out: splitLpFees(fee, kind, { lockRatioBps }) });
  }
}
const locks = [];
for (let i = 0; i < 50; i += 1) {
  const allocation = i === 0 ? 0n : randBig(1n, 10n ** 15n);
  const lock = new CreatorLock(allocation);
  const updates = Array.from({ length: 20 }, () => randBig(0n, allocation * 2n + 1n));
  locks.push({ allocation, updates, ratios: updates.map((u) => lock.update(u)) });
}

// Markets: a Stampede, then random trades until breakout or the step limit, then breakout if reached.
const stateOf = (m) => ({
  v: m.curve.v,
  t: m.curve.t,
  k: m.curve.k,
  reserve: m.curve.reserve,
  sold: m.curve.sold,
  phase: m.phase,
  fees: { ...m.fees },
  feeTotal: m.feeTotal,
});
const markets = [];
for (let run = 0; run < 40; run += 1) {
  const pool = run % 5 === 0 ? 0n : randBig(NATIVE / 2n, 400n * NATIVE);
  const { market: m, settle: s } = marketAfterLobby(pool);
  const start = stateOf(m);
  const ops = [];
  let held = s.X;
  for (let step = 0; step < 150; step += 1) {
    const r = next();
    const hasInterface = next() < 0.7;
    let op;
    if (r < 0.4) op = { op: 'buyExactIn', gross: randBig(1n, 40n * NATIVE), minTokens: next() < 0.1 ? 10n ** 18n : 1n, hasInterface };
    else if (r < 0.7) op = { op: 'buyExactOut', tokens: randBig(1n, 90_000_000n * TOKEN), maxGross: next() < 0.1 ? 1n : 10n ** 18n, hasInterface };
    else op = { op: 'sell', tokens: held > 0n ? randBig(1n, held) : 1n, minReceived: next() < 0.1 ? 10n ** 18n : 0n, hasInterface };
    try {
      let res;
      if (op.op === 'buyExactIn') {
        res = m.buyExactIn(op.gross, op.minTokens, { hasInterface });
        held += res.tokens;
      } else if (op.op === 'buyExactOut') {
        res = m.buyExactOut(op.tokens, op.maxGross, { hasInterface });
        held += res.tokens;
      } else {
        res = m.sell(op.tokens, op.minReceived, { hasInterface });
        held -= op.tokens;
      }
      ops.push({ ...op, ok: true, result: res, state: stateOf(m) });
    } catch (e) {
      ops.push({ ...op, ok: false, error: e.message, state: stateOf(m) });
    }
    if (m.phase !== 'Curve') break;
  }
  const breakout = m.phase === 'BreakingOut' ? m.breakout('keeper') : null;
  markets.push({ pool, settle: s, start, ops, breakout, end: stateOf(m) });
}

const out = str({ params, settle, claims, fees, lpFees, locks, markets });
writeFileSync(new URL('../programs/herdcall-core/tests/vectors.json', import.meta.url), JSON.stringify(out));
const opsCount = markets.reduce((a, m) => a + m.ops.length, 0);
console.log(`vectors: ${settle.length} settle, ${claims.length} claims, ${fees.length} fee, ${lpFees.length} lp fee, ${locks.length} lock, ${markets.length} markets / ${opsCount} ops, ${markets.filter((m) => m.breakout).length} breakouts`);
