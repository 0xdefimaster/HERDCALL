// Automated checks for the invariants in document/STAMPEDE_SPEC.md §8. Run: npm run test:sim
import test from 'node:test';
import assert from 'node:assert/strict';
import { BPS, CurveState, EXAMPLE, NATIVE, TOKEN, State, exampleLobby, projectedPrice, settleMath } from './stampede.js';
import { STAMPEDE, projectStampede } from '../src/lib/stampede.js';

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
  // An empty Lobby is cancelled: deposits come back through refund.
  for (const addr of lobby.deposits.keys()) claims.set(addr, result.empty ? lobby.refund(addr) : lobby.claim(addr));
  return { lobby, result, claims };
}

function checkInvariants({ lobby, result, claims }) {
  const { X, N, F } = result;
  const D = lobby.total;
  const n = BigInt(lobby.deposits.size);
  const c = lobby.curve;
  // I1 supply bound
  assert.ok(2n * X < lobby.t0 && X <= lobby.curveSupply, 'I1: X < t0/2 and X <= curve supply');
  if (result.empty) {
    // I13 empty lobby: curve untouched, everyone refunded in full
    assert.equal(c.v, lobby.v0);
    assert.equal(c.t, lobby.t0);
    assert.equal(c.reserve, 0n);
    for (const [addr, cl] of claims) assert.equal(cl.refund, lobby.deposits.get(addr), 'I13: full refund');
    return;
  }
  // I2 re-anchored curve
  assert.equal(c.v, lobby.v0 + N, 'I2: v = v0 + N');
  assert.equal(c.t, lobby.t0 - X, 'I2: t = t0 - X');
  assert.equal(c.reserve, N, 'I2: reserve = N');
  assert.equal(c.sold, X, 'I2: sold = X');
  // I3 every deposited unit buys tokens or pays the fee
  assert.equal(N + F, D, 'I3: N + F = D');
  assert.ok(F * BPS <= D * lobby.feeBps + BPS, 'I3: fee within bps of gross');
  // I4 / I5 token dust bounded, no refunds
  let sumQ = 0n;
  let sumR = 0n;
  for (const cl of claims.values()) {
    sumQ += cl.tokens;
    sumR += cl.refund;
  }
  assert.ok(sumQ <= X && X - sumQ < n, 'I4: token dust < n');
  assert.equal(sumR, 0n, 'I5: no refunds when the lobby settles');
  // I6 conservation
  assert.equal(D, c.reserve + lobby.feesOut + lobby.paidNative + lobby.escrowNative, 'I6: native conservation');
  assert.equal(lobby.escrowToken + lobby.paidToken, X, 'I6: token conservation');
  // I7 one price for everyone, I12 granularity
  for (const [addr, cl] of claims) {
    const d = lobby.deposits.get(addr);
    assert.ok(cl.tokens >= 1n, 'I12: every depositor receives tokens');
    assert.ok(d * X >= cl.tokens * D, 'I7: nobody pays below the Lobby price');
    assert.ok(d * X < (cl.tokens + 1n) * D, 'I7: overpay below one token unit');
  }
  // I11 continuity: Lobby price equals the curve's opening spot price up to one rounding step
  const gap = N * c.t - X * c.v;
  assert.ok(gap >= 0n, 'I11: opening spot is never below the Lobby price');
  assert.ok(gap < lobby.v0 + 2n * N, 'I11: Lobby price exceeds opening spot by rounding only');
  // I16 solvency at settlement
  assert.ok(c.slack() >= 0n, 'I16: reserve covers a full sell-off');
  // I17 the live projection is exact
  const proj = projectedPrice(lobby, D);
  assert.equal(proj.tokens, X, 'I17: projected tokens == settled tokens');
}

test('settlement: no refunds at any demand level, Lobby share stays below half the virtual supply', () => {
  for (const sol of [1n, 5n, 20n, 50n, 100n, 186n, 1_000n, 10_000n]) {
    const run = runLobby([['a', (sol * NATIVE) / 2n], ['b', (sol * NATIVE) / 2n]]);
    assert.equal(run.result.empty, false);
    checkInvariants(run);
  }
});

test('continuity: the curve opens exactly at the price the Lobby paid', () => {
  const run = runLobby([['a', 50n * NATIVE]]);
  const { N, X } = run.result;
  const c = run.lobby.curve;
  // Compare N/X with v/t as floats; must agree to ~1e-9 relative.
  const lobbyPrice = Number(N) / Number(X);
  const spot = Number(c.v) / Number(c.t);
  assert.ok(Math.abs(lobbyPrice - spot) / spot < 1e-9);
  // The very next buy pays more than the Lobby price per token.
  const dt = 1_000_000n * TOKEN;
  assert.ok(c.quoteBuy(dt) * X > N * dt, 'first post-bell buyer pays above the Lobby price');
});

test('zero and below-minimum demand: empty settlement, full refunds, curve untouched', () => {
  const zero = exampleLobby();
  assert.equal(zero.settle(AT_SETTLE).empty, true);
  assert.equal(zero.curve.reserve, 0n);
  const run = runLobby([['a', EXAMPLE.minDeposit], ['b', EXAMPLE.minDeposit]]);
  assert.equal(run.result.empty, true);
  assert.equal(run.lobby.terminal, State.Cancelled);
  assert.throws(() => run.lobby.claim('a'), /not settled/);
  checkInvariants(run);
});

test('fuzz: invariants hold across random lobbies and fees', () => {
  for (let seed = 1; seed <= 300; seed += 1) {
    const next = rng(seed);
    const feeBps = randBig(next, 0n, 300n);
    const n = 1 + Math.floor(next() * 150);
    const whale = next() < 0.3;
    const deposits = [];
    for (let i = 0; i < n; i += 1) {
      const hi = whale && i === 0 ? 2_000n * NATIVE : 5n * NATIVE;
      deposits.push([`w${i}`, randBig(next, EXAMPLE.minDeposit, hi)]);
    }
    checkInvariants(runLobby(deposits, { feeBps }));
  }
});

test('solvency: random trading after the bell never makes the curve insolvent', () => {
  for (let seed = 1; seed <= 100; seed += 1) {
    const next = rng(1_000 + seed);
    const run = runLobby([['a', randBig(next, NATIVE, 500n * NATIVE)]]);
    const c = run.lobby.curve;
    let lastSlack = c.slack();
    for (let i = 0; i < 200; i += 1) {
      if (next() < 0.55) {
        const room = c.t - 1n;
        const dt = randBig(next, TOKEN, room / 50n > TOKEN + 1n ? room / 50n : TOKEN + 1n);
        c.buy(dt);
      } else if (c.sold > 0n) {
        const dt = randBig(next, 1n, c.sold + 1n);
        c.sell(dt > c.sold ? c.sold : dt);
      }
      assert.ok(c.v * c.t >= c.k, 'I16: rounding never lets the curve product fall below k');
      const s = c.slack();
      assert.ok(s >= 0n, 'I16: slack never negative');
      assert.ok(s >= lastSlack, 'I16: rounding only adds to the slack');
      lastSlack = s;
    }
    // Everyone sells everything: must succeed and leave a non-negative reserve.
    if (c.sold > 0n) c.sell(c.sold);
    assert.ok(c.reserve >= 0n);
  }
});

test('order independence: deposit order does not change any allocation', () => {
  const next = rng(42);
  const deposits = Array.from({ length: 60 }, (_, i) => [`w${i}`, randBig(next, EXAMPLE.minDeposit, 3n * NATIVE)]);
  const forward = runLobby(deposits);
  const reversed = runLobby([...deposits].reverse());
  for (const [addr, c] of forward.claims) assert.deepEqual(reversed.claims.get(addr), c);
});

test('split neutrality: splitting one deposit into k wallets gains less than k token units', () => {
  const others = Array.from({ length: 20 }, (_, i) => [`o${i}`, 2n * NATIVE]);
  for (const k of [2n, 10n, 100n]) {
    const whole = runLobby([['whale', 100n * NATIVE], ...others]);
    const parts = Array.from({ length: Number(k) }, (_, i) => [`split${i}`, (100n * NATIVE) / k]);
    const split = runLobby([...parts, ...others]);
    let splitTokens = 0n;
    for (let i = 0; i < Number(k); i += 1) splitTokens += split.claims.get(`split${i}`).tokens;
    const w = whole.claims.get('whale').tokens;
    const diff = splitTokens > w ? splitTokens - w : w - splitTokens;
    assert.ok(diff < k, `k=${k}: only rounding differs`);
  }
});

test('live projection: shown price equals the settled price for the same pool', () => {
  const lobby = exampleLobby();
  lobby.deposit('a', 7n * NATIVE, OPEN);
  lobby.deposit('b', 13n * NATIVE, OPEN);
  const shown = projectedPrice(lobby, lobby.total);
  const r = lobby.settle(AT_SETTLE);
  assert.equal(shown.tokens, r.X);
  assert.equal(shown.netPerToken.num, r.N);
  assert.equal(projectedPrice(lobby, EXAMPLE.minTotal - 1n), null, 'no projection below the minimum');
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
  assert.ok(dust.token < 2n && dust.native === 0n, 'only token dust, below participant count');
  assert.equal(lobby.escrowToken, 0n);
  assert.equal(lobby.escrowNative, 0n);
  assert.throws(() => lobby.sweepDust(), /already swept/);
});

test('granularity guard: settlement refuses a lobby where a valid deposit would get zero tokens', () => {
  // Pathological parameters: a curve with almost no tokens.
  const lobby = exampleLobby({ t0: 40n, curveSupply: 20n });
  lobby.deposit('a', NATIVE, OPEN);
  lobby.deposit('b', NATIVE, OPEN);
  assert.throws(() => lobby.settle(AT_SETTLE), /granularity/);
  lobby.cancel(AFTER_DEADLINE);
  assert.equal(lobby.refund('a').refund, NATIVE, 'funds are recoverable through cancellation');
  assert.equal(lobby.refund('b').refund, NATIVE);
});

test('caps: minimum deposit, lobby total and per-wallet cap are enforced', () => {
  const lobby = exampleLobby({ maxTotal: 3n * NATIVE, maxPerWallet: 2n * NATIVE });
  assert.throws(() => lobby.deposit('a', EXAMPLE.minDeposit - 1n, OPEN), /minimum/);
  lobby.deposit('a', 2n * NATIVE, OPEN);
  assert.throws(() => lobby.deposit('a', EXAMPLE.minDeposit, OPEN), /wallet cap/);
  lobby.deposit('b', NATIVE, OPEN);
  assert.throws(() => lobby.deposit('c', EXAMPLE.minDeposit, OPEN), /lobby cap/);
});

test('parameters: a curve supply below t0/2 is rejected', () => {
  assert.throws(() => exampleLobby({ curveSupply: EXAMPLE.t0 / 2n - 1n }), /curveSupply/);
});

test('settleMath matches the Lobby object', () => {
  const lobby = exampleLobby();
  lobby.deposit('a', 12n * NATIVE, OPEN);
  const expected = settleMath(lobby, 12n * NATIVE);
  assert.deepEqual(lobby.settle(AT_SETTLE), expected);
});

test('CurveState: buying then selling the same amount never returns more than was paid', () => {
  const c = new CurveState({ v: EXAMPLE.v0, t: EXAMPLE.t0 });
  const next = rng(9);
  for (let i = 0; i < 500; i += 1) {
    const dt = randBig(next, 1n, 50_000_000n * TOKEN);
    const paid = c.buy(dt);
    assert.ok(c.v * c.t >= c.k, 'product never below k after a buy');
    const back = c.sell(dt);
    assert.ok(c.v * c.t >= c.k, 'product never below k after a sell');
    assert.ok(back <= paid);
  }
  assert.ok(c.reserve >= 0n);
});

test('UI gauge (src/lib/stampede.js) matches the settlement model', () => {
  assert.equal(STAMPEDE.virtualSol, Number(EXAMPLE.v0 / NATIVE));
  assert.equal(STAMPEDE.virtualTokens, Number(EXAMPLE.t0 / TOKEN));
  assert.equal(STAMPEDE.totalSupply, Number(EXAMPLE.totalSupply / TOKEN));
  assert.equal(STAMPEDE.feeBps, Number(EXAMPLE.feeBps));
  for (const sol of [1n, 50n, 186n, 10_000n]) {
    const r = settleMath(EXAMPLE, sol * NATIVE);
    const ui = projectStampede(Number(sol));
    const exact = Number(r.X) / Number(TOKEN);
    assert.ok(Math.abs(ui.tokens - exact) / exact < 1e-9, `tokens at ${sol}`);
  }
});
