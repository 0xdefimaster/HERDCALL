// Automated checks for the invariants in document/STAMPEDE_SPEC.md §8. Run: npm run test:sim
import test from 'node:test';
import assert from 'node:assert/strict';
import { BPS, EXAMPLE, NATIVE, TOKEN, State, exampleLobby, makeCurve, settleMath } from './stampede.js';

const OPEN = 0;
const AT_SETTLE = 60;
const AFTER_DEADLINE = 400;

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

function randBig(next, lo, hi) {
  const span = Number(hi - lo);
  return lo + BigInt(Math.floor(next() * span));
}

function runLobby(deposits, overrides = {}) {
  const lobby = exampleLobby(overrides);
  for (const [addr, amount] of deposits) lobby.deposit(addr, amount, OPEN);
  const result = lobby.settle(AT_SETTLE);
  const claims = new Map();
  for (const addr of lobby.deposits.keys()) claims.set(addr, lobby.claim(addr));
  return { lobby, result, claims };
}

function checkInvariants({ lobby, result, claims }) {
  const { X, U, F, G, E } = result;
  const D = lobby.total;
  const n = BigInt(lobby.deposits.size);
  // I1 supply cap
  assert.ok(X <= lobby.lobbySupply, 'I1: X <= S_L');
  // I2 curve consistency
  if (!result.empty) assert.equal(lobby.curveReserve, lobby.curve.costOf(lobby.curveSold), 'I2: reserve == C(sold)');
  else assert.equal(lobby.curveReserve, 0n, 'I2: empty lobby leaves curve untouched');
  // I3 spend bound
  assert.ok(G <= D && G === U + F && E === D - G, 'I3: G = U + F <= D');
  if (!result.empty) assert.ok(F * BPS <= G * lobby.feeBps + BPS, 'I3: fee within bps of gross');
  // I4 / I5 rounding dust bounded by participant count
  let sumQ = 0n;
  let sumR = 0n;
  for (const c of claims.values()) {
    sumQ += c.tokens;
    sumR += c.refund;
  }
  assert.ok(sumQ <= X && X - sumQ < n, 'I4: token dust < n');
  assert.ok(sumR <= E && E - sumR < n, 'I5: native dust < n');
  // I6 conservation: every deposited unit is in reserve, fees, refunds or escrow dust
  assert.equal(D, lobby.curveReserve + lobby.feesOut + lobby.paidNative + lobby.escrowNative, 'I6: native conservation');
  assert.equal(lobby.escrowToken + lobby.paidToken, X, 'I6: token conservation');
  // I7 uniform price within one token unit / one native unit, I12 granularity
  for (const [addr, c] of claims) {
    const d = lobby.deposits.get(addr);
    if (result.empty) {
      assert.equal(c.refund, d, 'I13: empty lobby refunds in full');
      continue;
    }
    assert.ok(c.tokens >= 1n, 'I12: every depositor receives tokens');
    const spent = d - c.refund;
    assert.ok(spent * X >= c.tokens * G, 'I7: nobody pays below the clearing price');
    assert.ok(spent * X < (c.tokens + 1n) * G + X, 'I7: overpay bounded by rounding');
  }
}

test('curve: costOf / maxTokensFor are exact inverses at the boundary', () => {
  const curve = makeCurve(EXAMPLE.curve);
  assert.equal(curve.costOf(0n), 0n);
  const next = rng(7);
  for (let i = 0; i < 2000; i += 1) {
    const budget = randBig(next, 0n, 500n * NATIVE);
    const x = curve.maxTokensFor(budget);
    assert.ok(curve.costOf(x) <= budget, 'C(x) fits the budget');
    assert.ok(curve.costOf(x + 1n) > budget, 'x is maximal');
  }
});

test('oversubscribed: all of S_L is sold and the excess is refunded pro-rata', () => {
  const run = runLobby([
    ['a', 50n * NATIVE],
    ['b', 30n * NATIVE],
    ['c', 20n * NATIVE],
  ]);
  const { result, lobby } = run;
  assert.equal(result.oversubscribed, true);
  assert.equal(result.X, lobby.lobbySupply);
  assert.equal(result.U, lobby.curve.costOf(lobby.lobbySupply));
  checkInvariants(run);
  const a = run.claims.get('a');
  const c = run.claims.get('c');
  assert.ok(a.tokens > c.tokens * 2n && a.refund > c.refund * 2n, 'shares follow deposits');
});

test('undersubscribed: X is the largest affordable amount and S_L is not exceeded', () => {
  const run = runLobby([
    ['a', NATIVE],
    ['b', 2n * NATIVE],
  ]);
  const { result, lobby } = run;
  assert.equal(result.oversubscribed, false);
  assert.ok(result.X < lobby.lobbySupply);
  assert.ok(lobby.curve.costOf(result.X + 1n) > result.netMax, 'I11: X is maximal for net demand');
  checkInvariants(run);
});

test('boundary: net demand exactly C(S_L)', () => {
  const curve = makeCurve(EXAMPLE.curve);
  const need = curve.costOf(EXAMPLE.lobbySupply);
  // Smallest gross deposit whose net (after fee) reaches C(S_L).
  let gross = (need * BPS + (BPS - EXAMPLE.feeBps) - 1n) / (BPS - EXAMPLE.feeBps);
  while ((gross * (BPS - EXAMPLE.feeBps)) / BPS < need) gross += 1n;
  const run = runLobby([['a', gross]]);
  assert.equal(run.result.X, EXAMPLE.lobbySupply);
  checkInvariants(run);
  const below = runLobby([['a', gross - 1n]]);
  assert.ok(below.result.X < EXAMPLE.lobbySupply, 'one unit less cannot clear all of S_L');
  checkInvariants(below);
});

test('zero and below-minimum demand: empty settlement, full refunds, curve untouched', () => {
  const zero = exampleLobby();
  const r0 = zero.settle(AT_SETTLE);
  assert.equal(r0.empty, true);
  assert.equal(zero.curveSold, 0n);
  assert.equal(zero.curveReserve, 0n);

  const run = runLobby([['a', EXAMPLE.minDeposit], ['b', EXAMPLE.minDeposit]]);
  assert.equal(run.result.empty, true);
  checkInvariants(run);
});

test('fuzz: invariants hold across random lobbies, fees and supplies', () => {
  for (let seed = 1; seed <= 300; seed += 1) {
    const next = rng(seed);
    const lobbySupply = randBig(next, 10_000_000n * TOKEN, 800_000_000n * TOKEN);
    const feeBps = randBig(next, 0n, 300n);
    const n = 1 + Math.floor(next() * 150);
    const whale = next() < 0.3;
    const deposits = [];
    for (let i = 0; i < n; i += 1) {
      const hi = whale && i === 0 ? 2_000n * NATIVE : 5n * NATIVE;
      deposits.push([`w${i}`, randBig(next, EXAMPLE.minDeposit, hi)]);
    }
    checkInvariants(runLobby(deposits, { lobbySupply, feeBps }));
  }
});

test('order independence: deposit order does not change any allocation', () => {
  const next = rng(42);
  const deposits = Array.from({ length: 60 }, (_, i) => [`w${i}`, randBig(next, EXAMPLE.minDeposit, 3n * NATIVE)]);
  const forward = runLobby(deposits);
  const reversed = runLobby([...deposits].reverse());
  for (const [addr, c] of forward.claims) assert.deepEqual(reversed.claims.get(addr), c);
});

test('split neutrality: splitting one deposit into k wallets gains less than k units', () => {
  const others = Array.from({ length: 20 }, (_, i) => [`o${i}`, 2n * NATIVE]);
  for (const k of [2n, 10n, 100n]) {
    const whole = runLobby([['whale', 100n * NATIVE], ...others]);
    const parts = Array.from({ length: Number(k) }, (_, i) => [`split${i}`, (100n * NATIVE) / k]);
    const split = runLobby([...parts, ...others]);
    let splitTokens = 0n;
    let splitRefund = 0n;
    for (let i = 0; i < Number(k); i += 1) {
      splitTokens += split.claims.get(`split${i}`).tokens;
      splitRefund += split.claims.get(`split${i}`).refund;
    }
    const w = whole.claims.get('whale');
    const dTok = splitTokens > w.tokens ? splitTokens - w.tokens : w.tokens - splitTokens;
    const dRef = splitRefund > w.refund ? splitRefund - w.refund : w.refund - splitRefund;
    assert.ok(dTok < k && dRef < k, `k=${k}: only rounding differs`);
  }
});

test('state machine: time windows are disjoint and terminal states are final', () => {
  const lobby = exampleLobby();
  lobby.deposit('a', NATIVE, OPEN);
  assert.equal(lobby.state(OPEN), State.Open);
  assert.throws(() => lobby.settle(OPEN), /not settleable/);
  assert.equal(lobby.state(AT_SETTLE), State.Settling);
  assert.throws(() => lobby.deposit('b', NATIVE, AT_SETTLE), /not open/);
  assert.throws(() => lobby.cancel(AT_SETTLE), /before deadline/);
  assert.throws(() => lobby.settle(AFTER_DEADLINE), /deadline passed/);
  assert.throws(() => lobby.refund('a'), /not cancelled/);

  lobby.cancel(AFTER_DEADLINE);
  assert.equal(lobby.state(AFTER_DEADLINE), State.Cancelled);
  assert.throws(() => lobby.settle(AT_SETTLE), /not settleable/, 'a cancelled lobby can never settle');
  assert.throws(() => lobby.claim('a'), /not settled/);
  assert.equal(lobby.refund('a').refund, NATIVE);
  assert.equal(lobby.escrowNative, 0n, 'cancelled lobby returns every deposit');

  const settled = exampleLobby();
  settled.deposit('a', NATIVE, OPEN);
  settled.settle(AT_SETTLE);
  assert.throws(() => settled.cancel(AFTER_DEADLINE), /already final/, 'a settled lobby can never be cancelled');
  assert.throws(() => settled.refund('a'), /not cancelled/);
});

test('claims: no double withdrawal, re-entrant claim fails, dust sweep only after all claims', () => {
  const lobby = exampleLobby();
  lobby.deposit('a', 3n * NATIVE, OPEN);
  lobby.deposit('b', 7n * NATIVE, OPEN);
  lobby.deposit('a', 1n * NATIVE, OPEN); // repeat deposits aggregate per address
  lobby.settle(AT_SETTLE);

  let reentered = null;
  lobby.claim('a', () => {
    try {
      lobby.claim('a');
    } catch (err) {
      reentered = err.message;
    }
  });
  assert.equal(reentered, 'already withdrawn', 're-entrant claim is rejected');
  assert.throws(() => lobby.claim('a'), /already withdrawn/);
  assert.throws(() => lobby.claim('nobody'), /nothing to claim/);
  assert.throws(() => lobby.sweepDust(), /claims outstanding/);
  lobby.claim('b');
  const dust = lobby.sweepDust();
  assert.ok(dust.token < 2n && dust.native < 2n, 'dust is below participant count');
  assert.equal(lobby.escrowToken, 0n);
  assert.equal(lobby.escrowNative, 0n);
  assert.throws(() => lobby.sweepDust(), /already swept/);
});

test('granularity guard: settlement refuses a lobby where a valid deposit would get zero tokens', () => {
  // Pathological parameters: a 1-unit Lobby supply against many deposits.
  const lobby = exampleLobby({ lobbySupply: 1n });
  lobby.deposit('a', NATIVE, OPEN);
  lobby.deposit('b', NATIVE, OPEN);
  assert.throws(() => lobby.settle(AT_SETTLE), /granularity/);
  lobby.cancel(AFTER_DEADLINE);
  assert.equal(lobby.refund('a').refund, NATIVE, 'funds are recoverable through cancellation');
  assert.equal(lobby.refund('b').refund, NATIVE);
});

test('lobby cap and minimum deposit are enforced', () => {
  const lobby = exampleLobby({ maxTotal: 2n * NATIVE });
  assert.throws(() => lobby.deposit('a', EXAMPLE.minDeposit - 1n, OPEN), /minimum/);
  lobby.deposit('a', 2n * NATIVE, OPEN);
  assert.throws(() => lobby.deposit('b', EXAMPLE.minDeposit, OPEN), /cap/);
});

test('settleMath matches the Lobby object', () => {
  const lobby = exampleLobby();
  lobby.deposit('a', 12n * NATIVE, OPEN);
  const expected = settleMath(lobby, 12n * NATIVE);
  assert.deepEqual(lobby.settle(AT_SETTLE), expected);
});
