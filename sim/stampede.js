// Reference model of the Lobby → Stampede settlement in document/STAMPEDE_SPEC.md (v0.2).
// Integer math only (BigInt): native amounts in base units (lamports / wei), tokens in base units.
// This is a specification aid, not contract code.
//
// Model: the Lobby pool makes one buy when the countdown ends. Every participant gets tokens in
// proportion to their deposit, and everyone pays one price: the price the curve opens at.

export const BPS = 10_000n;

export function ceilDiv(a, b) {
  return (a + b - 1n) / b;
}

// Constant-product curve state with virtual reserves.
//   v, t     virtual native / token reserves (price = v / t)
//   k        invariant v * t (fixed after the Lobby settles)
//   reserve  real native held by the curve
//   sold     tokens in circulation that came from the curve (Lobby allocation included)
export class CurveState {
  constructor({ v, t, reserve = 0n, sold = 0n }) {
    if (v <= 0n || t <= 0n) throw new Error('virtual reserves must be positive');
    Object.assign(this, { v, t, k: v * t, reserve, sold });
  }

  // Native cost (rounded up) to buy dt tokens.
  quoteBuy(dt) {
    if (dt <= 0n || dt >= this.t) throw new Error('dt out of range');
    return ceilDiv(this.k, this.t - dt) - this.v;
  }

  // Native returned (rounded down) for selling dt tokens.
  quoteSell(dt) {
    if (dt <= 0n || dt > this.sold) throw new Error('dt out of range');
    return this.v - ceilDiv(this.k, this.t + dt);
  }

  buy(dt) {
    const cost = this.quoteBuy(dt);
    this.v += cost;
    this.t -= dt;
    this.reserve += cost;
    this.sold += dt;
    return cost;
  }

  sell(dt) {
    const out = this.quoteSell(dt);
    if (out > this.reserve) throw new Error('insolvent');
    this.v -= out;
    this.t += dt;
    this.reserve -= out;
    this.sold -= dt;
    return out;
  }

  // Native needed to buy back every sold token right now (worst-case sell-off).
  sellAllPayout() {
    return this.sold === 0n ? 0n : this.v - ceilDiv(this.k, this.t + this.sold);
  }

  // Solvency slack: reserve minus the worst-case sell-off payout. Must stay >= 0.
  slack() {
    return this.reserve - this.sellAllPayout();
  }

  spotPrice() {
    return { num: this.v, den: this.t };
  }
}

// Pure settlement math for total deposits D (spec §4).
export function settleMath({ v0, t0, curveSupply, feeBps, minTotal }, total) {
  if (total < minTotal) {
    return { empty: true, X: 0n, N: 0n, F: 0n, refund: total };
  }
  const N = (total * (BPS - feeBps)) / BPS; // net native that buys tokens
  const F = total - N; // fee
  const X = (N * t0) / (v0 + 2n * N); // tokens to the Lobby (always < t0 / 2)
  if (X === 0n) return { empty: true, X: 0n, N: 0n, F: 0n, refund: total };
  if (X > curveSupply) throw new Error('curve supply too small for this curve');
  return { empty: false, X, N, F, refund: 0n };
}

// The price everyone would get if the bell rang now (spec §6). Shown live during the countdown.
export function projectedPrice(params, total) {
  const r = settleMath(params, total);
  if (r.empty) return null;
  return { netPerToken: { num: r.N, den: r.X }, grossPerToken: { num: total, den: r.X }, tokens: r.X };
}

export const State = Object.freeze({ Open: 'Open', Settling: 'Settling', Settled: 'Settled', Cancelled: 'Cancelled' });

export class Lobby {
  constructor({ v0, t0, curveSupply, feeBps, closeAt, settleDeadline, minDeposit, minTotal, maxTotal, maxPerWallet = 0n }) {
    if (!(closeAt < settleDeadline)) throw new Error('closeAt must be before settleDeadline');
    // The Lobby can take up to t0/2 tokens; the curve must hold more than that.
    if (2n * curveSupply < t0) throw new Error('curveSupply must be at least t0 / 2');
    Object.assign(this, { v0, t0, curveSupply, feeBps, closeAt, settleDeadline, minDeposit, minTotal, maxTotal, maxPerWallet });
    this.terminal = null; // 'Settled' | 'Cancelled' once reached; never changes afterwards
    this.deposits = new Map();
    this.withdrawn = new Set();
    this.total = 0n;
    this.result = null;
    this.curve = null; // CurveState created at settlement
    this.escrowNative = 0n;
    this.escrowToken = 0n;
    this.feesOut = 0n;
    this.paidNative = 0n;
    this.paidToken = 0n;
    this.dustSwept = null;
  }

  state(now) {
    if (this.terminal) return this.terminal;
    return now < this.closeAt ? State.Open : State.Settling;
  }

  deposit(addr, amount, now) {
    if (this.state(now) !== State.Open) throw new Error('lobby not open');
    if (amount < this.minDeposit) throw new Error('below minimum deposit');
    if (this.total + amount > this.maxTotal) throw new Error('lobby cap reached');
    const next = (this.deposits.get(addr) ?? 0n) + amount;
    if (this.maxPerWallet > 0n && next > this.maxPerWallet) throw new Error('wallet cap reached');
    this.deposits.set(addr, next);
    this.total += amount;
    this.escrowNative += amount;
  }

  settle(now) {
    if (this.state(now) !== State.Settling) throw new Error('not settleable');
    if (now >= this.settleDeadline) throw new Error('settle deadline passed');
    const r = settleMath(this, this.total);
    // Guarantee: every accepted deposit (>= minDeposit) receives at least one token unit.
    if (!r.empty && r.X * this.minDeposit < this.total) throw new Error('allocation granularity violated');
    this.result = r;
    if (r.empty) {
      this.curve = new CurveState({ v: this.v0, t: this.t0 });
    } else {
      // Re-anchored curve: opens at the Lobby price, holds the Lobby's net native as real reserve.
      this.curve = new CurveState({ v: this.v0 + r.N, t: this.t0 - r.X, reserve: r.N, sold: r.X });
      this.escrowNative -= r.N + r.F;
      this.feesOut = r.F;
      this.escrowToken = r.X;
    }
    this.terminal = State.Settled;
    return r;
  }

  cancel(now) {
    if (this.terminal) throw new Error('already final');
    if (now < this.settleDeadline) throw new Error('cannot cancel before deadline');
    this.terminal = State.Cancelled;
  }

  // Pull claim after settlement. Effects are recorded before the external transfer (CEI).
  claim(addr, onTransfer) {
    if (this.terminal !== State.Settled) throw new Error('not settled');
    const d = this.deposits.get(addr) ?? 0n;
    if (d === 0n) throw new Error('nothing to claim');
    if (this.withdrawn.has(addr)) throw new Error('already withdrawn');
    this.withdrawn.add(addr);
    const { X, empty } = this.result;
    const q = (d * X) / this.total;
    const r = empty ? d : 0n;
    this.escrowToken -= q;
    this.escrowNative -= r;
    this.paidToken += q;
    this.paidNative += r;
    onTransfer?.(addr, q, r);
    return { tokens: q, refund: r };
  }

  refund(addr, onTransfer) {
    if (this.terminal !== State.Cancelled) throw new Error('not cancelled');
    const d = this.deposits.get(addr) ?? 0n;
    if (d === 0n) throw new Error('nothing to refund');
    if (this.withdrawn.has(addr)) throw new Error('already withdrawn');
    this.withdrawn.add(addr);
    this.escrowNative -= d;
    this.paidNative += d;
    onTransfer?.(addr, 0n, d);
    return { tokens: 0n, refund: d };
  }

  // Token rounding dust becomes sweepable only once every participant has withdrawn.
  sweepDust() {
    if (this.terminal !== State.Settled) throw new Error('not settled');
    if (this.withdrawn.size !== this.deposits.size) throw new Error('claims outstanding');
    if (this.dustSwept) throw new Error('already swept');
    this.dustSwept = { token: this.escrowToken, native: this.escrowNative };
    this.escrowToken = 0n;
    this.escrowNative = 0n;
    return this.dustSwept;
  }
}

// Example parameters (pump.fun-like curve, 6-decimal token, 9-decimal native).
export const TOKEN = 1_000_000n;
export const NATIVE = 1_000_000_000n;
export const EXAMPLE = Object.freeze({
  v0: 30n * NATIVE,
  t0: 1_073_000_000n * TOKEN,
  totalSupply: 1_000_000_000n * TOKEN,
  curveSupply: 800_000_000n * TOKEN,
  feeBps: 100n,
  minDeposit: NATIVE / 100n, // 0.01
  minTotal: NATIVE / 10n, // 0.1
  maxTotal: 10_000n * NATIVE,
  maxPerWallet: 0n, // 0 = no per-wallet cap
});

export function exampleLobby(overrides = {}) {
  const p = { ...EXAMPLE, ...overrides };
  return new Lobby({
    v0: p.v0,
    t0: p.t0,
    curveSupply: p.curveSupply,
    feeBps: p.feeBps,
    closeAt: 30,
    settleDeadline: 330,
    minDeposit: p.minDeposit,
    minTotal: p.minTotal,
    maxTotal: p.maxTotal,
    maxPerWallet: p.maxPerWallet,
  });
}
