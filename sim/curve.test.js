// Checks for document/CURVE_SPEC.md §7. Run: npm run test:sim
import test from 'node:test';
import assert from 'node:assert/strict';
import { NATIVE, TOKEN } from './stampede.js';
import { CURVE_EXAMPLE, CreatorLock, Market, Phase, costToBreakout, feeOn, marketAfterLobby, splitFee, splitLpFees } from './curve.js';

const BIG = 10n ** 30n;

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
const randBig = (next, lo, hi) => lo + (BigInt(Math.floor(next() * 2 ** 52)) * (hi - lo)) / 2n ** 52n;

// C1-C6 after every step.
function check(m, holders) {
  const c = m.curve;
  assert.equal(c.reserve, c.v - m.v0, 'C1 reserve = v - v0');
  assert.equal(c.t + c.sold, m.t0, 'C2 t + sold = t0');
  assert.ok(c.v * c.t >= c.k, 'C3 v·t >= k');
  assert.ok(c.slack() >= 0n, 'C4 solvency');
  assert.ok(c.sold <= m.curveSupply, 'C5 sold <= curve supply');
  const parts = Object.values(m.fees).reduce((a, b) => a + b, 0n);
  assert.equal(parts, m.feeTotal, 'C6 fee parts add up');
  assert.equal(m.flows.paidIn - m.flows.paidOut, c.reserve + m.feeTotal, 'C7 native conservation');
  const held = [...holders.values()].reduce((a, b) => a + b, 0n);
  assert.equal(held, c.sold, 'C8 tokens held = tokens sold');
}

test('random trading until breakout keeps every invariant', () => {
  const next = rng(1);
  let breakouts = 0;
  for (let run = 0; run < 120; run += 1) {
    const pool = next() < 0.2 ? 0n : randBig(next, NATIVE / 2n, 400n * NATIVE);
    const { market: m, settle } = marketAfterLobby(pool);
    const holders = new Map([['lobby', settle.X]]);
    check(m, holders);
    for (let step = 0; step < 400 && m.phase === Phase.Curve; step += 1) {
      const who = `w${Math.floor(next() * 8)}`;
      const held = holders.get(who) ?? 0n;
      const r = next();
      if (r < 0.45) {
        const q = m.buyExactIn(randBig(next, NATIVE / 100n, 30n * NATIVE), 1n);
        holders.set(who, held + q.tokens);
      } else if (r < 0.7) {
        const q = m.buyExactOut(randBig(next, TOKEN, 60_000_000n * TOKEN), BIG);
        holders.set(who, held + q.tokens);
      } else if (held > 0n) {
        const dt = randBig(next, 1n, held);
        m.sell(dt, 0n);
        holders.set(who, held - dt);
      }
      check(m, holders);
    }
    if (m.phase === Phase.BreakingOut) breakouts += 1;
  }
  assert.ok(breakouts > 20, `some runs reach breakout (${breakouts})`);
});

test('the buy that crosses the curve supply is clamped, then trading closes', () => {
  const { market: m } = marketAfterLobby(20n * NATIVE);
  const left = m.remaining();
  const q = m.buyExactOut(left + 5_000_000n * TOKEN, BIG);
  assert.equal(q.tokens, left);
  assert.equal(q.clamped, true);
  assert.equal(m.phase, Phase.BreakingOut);
  assert.throws(() => m.buyExactIn(NATIVE, 1n), /trading closed/);
  assert.throws(() => m.sell(1n, 0n), /trading closed/);

  const { market: m2 } = marketAfterLobby(20n * NATIVE);
  const left2 = m2.remaining();
  const need = costToBreakout(m2);
  const gross = need + feeOn(need, m2.feeBps) + 7n * NATIVE;
  const q2 = m2.buyExactIn(gross, 1n);
  assert.equal(q2.tokens, left2);
  assert.ok(q2.refund >= 7n * NATIVE - 1n, 'the unused native comes back');
  assert.equal(q2.gross + q2.refund, gross);
});

test('slippage limits are enforced on both sides', () => {
  const { market: m } = marketAfterLobby(10n * NATIVE);
  const q = m.quoteBuyExactOut(10_000_000n * TOKEN);
  assert.throws(() => m.buyExactOut(10_000_000n * TOKEN, q.gross - 1n), /slippage/);
  m.buyExactOut(10_000_000n * TOKEN, q.gross);
  const s = m.quoteSell(5_000_000n * TOKEN);
  assert.throws(() => m.sell(5_000_000n * TOKEN, s.received + 1n), /slippage/);
  m.sell(5_000_000n * TOKEN, s.received);
  const e = m.quoteBuyExactIn(NATIVE);
  assert.throws(() => m.buyExactIn(NATIVE, e.tokens + 1n), /slippage/);
});

test('fees: ceil of 1% of notional, never in the reserve, split adds up, escrow half for bonded', () => {
  assert.equal(feeOn(1n, 100n), 1n);
  assert.equal(feeOn(10_000n, 100n), 100n);
  const next = rng(3);
  for (let i = 0; i < 2000; i += 1) {
    const fee = randBig(next, 0n, 10n ** 12n);
    for (const kind of ['bonded', 'unbonded']) {
      for (const hasInterface of [true, false]) {
        const p = splitFee(fee, kind, { hasInterface });
        assert.equal(Object.values(p).reduce((a, b) => a + b, 0n), fee);
        if (!hasInterface) assert.equal(p.interface, 0n);
        if (kind === 'unbonded') assert.equal(p.creatorEscrow, 0n);
        else assert.ok(p.creatorEscrow <= p.creatorNow + 1n && p.creatorNow <= p.creatorEscrow + 1n);
      }
    }
  }
});

test('breakout: permissionless, once, at the final curve price, supply conserved', () => {
  const next = rng(4);
  for (let i = 0; i < 60; i += 1) {
    const pool = next() < 0.2 ? 0n : randBig(next, NATIVE, 600n * NATIVE);
    const { market: m, settle } = marketAfterLobby(pool);
    assert.throws(() => m.breakout('early'), /not breaking out/);
    m.buyExactOut(m.remaining(), BIG);
    const reserve = m.curve.reserve;
    const r = m.breakout(`anyone${i}`);
    assert.equal(r.lpNative + r.keeperFee, reserve, 'all of the reserve moves');
    assert.equal(r.lpTokens + r.burned, m.lpSupply);
    assert.ok(r.lpTokens <= m.lpSupply);
    // DEX price >= final curve price, equal up to one token unit of rounding.
    assert.ok(r.dexPrice.num * r.finalSpot.den >= r.finalSpot.num * r.dexPrice.den, 'never opens below the curve');
    assert.ok(r.lpNative * r.finalSpot.den < (r.lpTokens + 1n) * r.finalSpot.num, 'and not more than one unit above');
    // supply: everything sold + LP + burned = total
    assert.equal(m.curve.sold + r.lpTokens + r.burned, m.totalSupply);
    assert.equal(m.phase, Phase.Migrated);
    assert.throws(() => m.breakout('again'), /not breaking out/);
    assert.ok(settle.X <= m.curveSupply);
  }
});

test('breakout reserve depends only on the Stampede, not on the trading path', () => {
  const next = rng(5);
  for (const pool of [0n, 30n * NATIVE, 186n * NATIVE]) {
    const direct = marketAfterLobby(pool).market;
    direct.buyExactOut(direct.remaining(), BIG);
    const target = direct.curve.reserve;
    for (let run = 0; run < 15; run += 1) {
      const { market: m } = marketAfterLobby(pool);
      let held = 0n;
      while (m.phase === Phase.Curve) {
        if (held > 0n && next() < 0.3) {
          const dt = randBig(next, 1n, held);
          m.sell(dt, 0n);
          held -= dt;
        } else {
          held += m.buyExactIn(randBig(next, NATIVE / 10n, 40n * NATIVE), 1n).tokens;
        }
      }
      const diff = m.curve.reserve - target;
      assert.ok(diff >= 0n && diff < 10_000n, `path changes the reserve only by rounding (${diff})`);
    }
  }
});

test('parameters: t0 above curveSupply + lpSupply is rejected; with it allowed, the DEX opens above the curve', () => {
  const pumpLike = { t0: 1_073_000_000n * TOKEN };
  assert.throws(() => marketAfterLobby(50n * NATIVE, pumpLike), /t0 must not exceed/);
  const { market: m } = marketAfterLobby(186n * NATIVE, pumpLike, { strictLp: false });
  m.buyExactOut(m.remaining(), BIG);
  const r = m.breakout('k');
  assert.equal(r.burned, 0n);
  assert.ok(r.dexPrice.num * r.finalSpot.den > r.finalSpot.num * r.dexPrice.den, 'LP short of tokens: price jumps up');
  assert.throws(() => new Market(m.curve, { ...CURVE_EXAMPLE, lpSupply: 100n }), /supply split/);
});

test('a dead token: every holder can still sell everything back; the Stampede surplus stays in the curve', () => {
  const { market: m, settle } = marketAfterLobby(80n * NATIVE);
  const surplus = m.curve.slack(); // reserve above the sell-everything payout, created by the one-price Stampede
  const a = m.buyExactIn(20n * NATIVE, 1n).tokens;
  const b = m.buyExactIn(5n * NATIVE, 1n).tokens;
  for (const dt of [settle.X, a, b]) m.sell(dt, 0n);
  assert.equal(m.curve.sold, 0n);
  assert.ok(m.curve.reserve >= surplus, 'the surplus never shrinks');
  assert.ok(surplus > 20n * NATIVE, 'and it is large: about N²/(V0+2N)');
});

test('creator lock ratio only goes down; locking again does not restore it', () => {
  const next = rng(6);
  for (let run = 0; run < 200; run += 1) {
    const allocation = randBig(next, 1n, 10n ** 15n);
    const lock = new CreatorLock(allocation);
    let lowest = 10_000n;
    for (let i = 0; i < 30; i += 1) {
      const locked = randBig(next, 0n, allocation * 2n);
      const r = lock.update(locked);
      const now = locked >= allocation ? 10_000n : (locked * 10_000n) / allocation;
      if (now < lowest) lowest = now;
      assert.equal(r, lowest);
    }
  }
  assert.equal(new CreatorLock(0n).ratioBps, 0n);
});

test('hold to earn: forfeited creator share goes to Rekt-back, on the curve and after breakout', () => {
  const next = rng(7);
  for (let i = 0; i < 2000; i += 1) {
    const fee = randBig(next, 0n, 10n ** 12n);
    const ratio = randBig(next, 0n, 10_000n);
    for (const kind of ['bonded', 'unbonded']) {
      const lp = splitLpFees(fee, kind, { lockRatioBps: ratio });
      assert.equal(lp.liquidity + lp.creator + lp.rektBack + lp.protocol, fee, 'LP fee parts add up');
      const full = splitLpFees(fee, kind);
      assert.equal(lp.liquidity, full.liquidity, 'liquidity share never changes');
      assert.equal(lp.protocol, full.protocol, 'protocol share never changes');
      assert.equal(lp.creator + lp.rektBack, full.creator + full.rektBack, 'only creator and Rekt-back trade places');
      if (kind === 'unbonded') assert.equal(lp.forfeited, 0n, 'unbonded creators keep their fixed share');
      const curve = splitFee(fee, kind, { lockRatioBps: ratio });
      const curveFull = splitFee(fee, kind);
      assert.equal(Object.values(curve).reduce((a, b) => a + b, 0n), fee);
      assert.equal(curve.protocol, curveFull.protocol);
      assert.equal(curve.interface, curveFull.interface);
    }
  }
  // Every creator gets at least pump.fun's 0.30% of volume on the curve.
  const vol = 1_000_000n;
  const fee = feeOn(vol, 100n);
  const unbonded = splitFee(fee, 'unbonded');
  const bonded = splitFee(fee, 'bonded');
  assert.equal(unbonded.creatorNow, (vol * 30n) / 10_000n);
  assert.equal(bonded.creatorNow + bonded.creatorEscrow, (vol * 50n) / 10_000n);
});
