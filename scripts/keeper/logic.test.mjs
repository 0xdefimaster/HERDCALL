import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULTS, addSample, band, decide, inBand, median, windowMedian } from './logic.mjs';

const P = 1n << 64n; // √price of 1
const fill = (from, to, step, sqrt = () => P) => {
  let s = [];
  for (let t = from; t <= to; t += step) s = addSample(s, t, sqrt(t), DEFAULTS.windowSecs);
  return s;
};

test('median of BigInts', () => {
  assert.equal(median([3n, 1n, 2n]), 2n);
  assert.equal(median([4n, 1n, 2n, 3n]), 2n);
  assert.equal(median([]), null);
});

test('the window drops old samples and needs enough coverage', () => {
  const s = fill(0, 3600, 30);
  assert.ok(s.every((x) => x.t > 3600 - DEFAULTS.windowSecs));
  assert.equal(windowMedian(s, 3600).median, P);
  // 10 minutes of history is not enough for a 30 minute median.
  assert.match(windowMedian(fill(3000, 3600, 30), 3600).reason, /span/);
  assert.match(windowMedian(fill(0, 300, 30), 300).reason, /samples/);
});

test('a short spike does not move the median', () => {
  const s = fill(0, 1800, 30, (t) => (t > 1700 ? P * 2n : P));
  assert.equal(windowMedian(s, 1800).median, P);
});

test('band is ±0.25% in √price, inside the program limit of 2% in price', () => {
  const [lo, hi] = band(P);
  assert.equal(hi - P, P / 400n);
  assert.ok(inBand([lo, hi], P) && !inBand([lo, hi], hi + 1n));
  const width = Number(hi * hi * 10_000n / (lo * lo)) - 10_000;
  assert.ok(width <= 200, `price width ${width} bps`);
});

test('reinvests only with a median, inside the band, once per interval', () => {
  const samples = fill(0, 1800, 30);
  const base = { now: 1800, pending: 20_000_000n, current: P, state: { samples } };
  assert.deepEqual(decide(base).reinvest.band, band(P));
  // Price pushed just before: no reinvest.
  assert.equal(decide({ ...base, current: P + P / 100n }).reinvest, null);
  assert.match(decide({ ...base, current: P + P / 100n }).skip, /outside the band/);
  // Below the minimum: nothing to do.
  assert.equal(decide({ ...base, pending: 9_999_999n }).reinvest, null);
  // Right after a reinvest: wait.
  assert.equal(decide({ ...base, state: { samples, lastReinvestAt: 1000 } }).reinvest, null);
  // Without history: wait.
  assert.match(decide({ ...base, state: { samples: [] } }).skip, /no median/);
});

test('claims on a schedule', () => {
  const base = { now: 100_000, pending: 0n, current: P, state: { samples: [] } };
  assert.equal(decide(base).claim, true);
  assert.equal(decide({ ...base, state: { samples: [], lastClaimAt: 100_000 - 3600 } }).claim, false);
  assert.equal(decide({ ...base, state: { samples: [], lastClaimAt: 100_000 - 6 * 3600 } }).claim, true);
});

test('alerts when the share waits more than a day, then every 6 hours', () => {
  let state = { samples: [] };
  const step = (now, pending = 20_000_000n) => {
    const r = decide({ now, pending, current: P, state });
    state = r.state;
    return r.alert;
  };
  assert.equal(step(0), null);
  assert.equal(step(24 * 3600), null);
  assert.match(step(24 * 3600 + 1), /waiting to be reinvested for 24h/);
  assert.equal(step(26 * 3600), null);
  assert.match(step(30 * 3600 + 1), /30h/);
  // Reinvested (below the minimum now): the clock resets.
  assert.equal(step(31 * 3600, 0n), null);
  assert.equal(state.pendingSince, null);
  assert.equal(step(32 * 3600), null);
});

test('does not try when the pool is too small for the minimum under the cap', () => {
  const samples = fill(0, 1800, 30);
  const base = { now: 1800, pending: 20_000_000n, current: P, state: { samples } };
  assert.match(decide({ ...base, cap: 7_000_000n }).skip, /pool too small/);
  assert.ok(decide({ ...base, cap: 10_000_000n }).reinvest);
});
