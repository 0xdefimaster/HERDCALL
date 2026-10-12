// Reference model of the Lobby → Stampede settlement in document/STAMPEDE_SPEC.md.
// Integer math only (BigInt): native amounts in base units (lamports / wei), tokens in base units.
// This is a specification aid, not contract code.

export const BPS = 10_000n;

export function ceilDiv(a, b) {
  return (a + b - 1n) / b;
}

// Constant-product bonding curve with virtual reserves (v0 native, t0 tokens), k = v0 * t0.
// C(x): net native reserve the curve must hold after x tokens are sold, rounded up (protocol-favourable).
export function makeCurve({ v0, t0 }) {
  if (v0 <= 0n || t0 <= 0n) throw new Error('virtual reserves must be positive');
  return {
    v0,
    t0,
    costOf(x) {
      if (x < 0n || x >= t0) throw new Error('x out of range');
      return ceilDiv(v0 * x, t0 - x);
    },
    // Largest x with costOf(x) <= n.
    maxTokensFor(n) {
      if (n < 0n) throw new Error('n must be >= 0');
      return (t0 * n) / (v0 + n);
    },
    // Marginal (spot) price after x tokens are sold, as a fraction native/token.
    spotPrice(x) {
      return { num: v0 * t0, den: (t0 - x) * (t0 - x) };
    },
  };
}

// Pure settlement math for total deposits D (spec §4).
export function settleMath({ curve, lobbySupply, feeBps, minTotal }, total) {
  if (total < minTotal) {
    return { empty: true, X: 0n, U: 0n, F: 0n, G: 0n, E: total, netMax: 0n };
  }
  const netMax = (total * (BPS - feeBps)) / BPS;
  const xFromDemand = curve.maxTokensFor(netMax);
  const X = xFromDemand < lobbySupply ? xFromDemand : lobbySupply;
  if (X === 0n) {
    return { empty: true, X: 0n, U: 0n, F: 0n, G: 0n, E: total, netMax };
  }
  const U = curve.costOf(X);
  const G = ceilDiv(U * BPS, BPS - feeBps);
  const F = G - U;
  const E = total - G;
  return { empty: false, X, U, F, G, E, netMax, oversubscribed: xFromDemand >= lobbySupply };
}

export const State = Object.freeze({ Open: 'Open', Settling: 'Settling', Settled: 'Settled', Cancelled: 'Cancelled' });

export class Lobby {
  constructor({ curve, lobbySupply, feeBps, closeAt, settleDeadline, minDeposit, minTotal, maxTotal }) {
    if (!(closeAt < settleDeadline)) throw new Error('closeAt must be before settleDeadline');
    if (lobbySupply <= 0n || lobbySupply >= curve.t0) throw new Error('lobbySupply out of range');
    Object.assign(this, { curve, lobbySupply, feeBps, closeAt, settleDeadline, minDeposit, minTotal, maxTotal });
    this.terminal = null; // 'Settled' | 'Cancelled' once reached; never changes afterwards
    this.deposits = new Map();
    this.withdrawn = new Set();
    this.total = 0n;
    this.result = null;
    // Balances held by the Lobby escrow and by the other recipients.
    this.escrowNative = 0n;
    this.escrowToken = 0n;
    this.curveReserve = 0n;
    this.curveSold = 0n;
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
    this.deposits.set(addr, (this.deposits.get(addr) ?? 0n) + amount);
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
    // Fund movements happen in one atomic step.
    this.escrowNative -= r.U + r.F;
    this.curveReserve = r.U;
    this.curveSold = r.X;
    this.feesOut = r.F;
    this.escrowToken = r.X; // X tokens minted/moved to escrow for claims
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
    const { X, E } = this.result;
    const q = (d * X) / this.total;
    const r = (d * E) / this.total;
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

  // Rounding dust becomes sweepable only once every participant has withdrawn.
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
  curve: { v0: 30n * NATIVE, t0: 1_073_000_000n * TOKEN },
  curveSupply: 800_000_000n * TOKEN,
  lobbySupply: 150_000_000n * TOKEN,
  feeBps: 100n,
  minDeposit: NATIVE / 100n, // 0.01
  minTotal: NATIVE / 10n, // 0.1
  maxTotal: 10_000n * NATIVE,
});

export function exampleLobby(overrides = {}) {
  const p = { ...EXAMPLE, ...overrides };
  return new Lobby({
    curve: makeCurve(p.curve),
    lobbySupply: p.lobbySupply,
    feeBps: p.feeBps,
    closeAt: 30,
    settleDeadline: 330,
    minDeposit: p.minDeposit,
    minTotal: p.minTotal,
    maxTotal: p.maxTotal,
  });
}
