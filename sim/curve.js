// Reference model of curve trading, fees and Breakout in document/CURVE_SPEC.md (v0.1).
// Integer math only (BigInt). Builds on CurveState from stampede.js. Specification aid, not contract code.
import { BPS, NATIVE, TOKEN, ceilDiv, exampleLobby } from './stampede.js';

export const Phase = Object.freeze({ Curve: 'Curve', BreakingOut: 'BreakingOut', Migrated: 'Migrated' });

// Fee split in bps of the fee (document/HERDCALL.md §5: 0.30 / 0.10 / 0.15 / 0.45 of the 1% for bonded tokens).
export const FEE_SPLIT = Object.freeze({
  bonded: { creator: 3000n, interface: 1000n, rektBack: 1500n, protocol: 4500n },
  unbonded: { creator: 1000n, interface: 1000n, rektBack: 3500n, protocol: 4500n },
});
export const CREATOR_ESCROW_BPS = 5000n; // bonded: half of the creator share waits in escrow

// Splits a fee. Each part rounds down; the protocol takes the remainder, so the parts always add up to the fee.
// Without an interface (direct trade) the interface share goes to the protocol.
export function splitFee(fee, kind = 'bonded', { hasInterface = true } = {}) {
  const s = FEE_SPLIT[kind];
  const creator = (fee * s.creator) / BPS;
  const iface = hasInterface ? (fee * s.interface) / BPS : 0n;
  const rektBack = (fee * s.rektBack) / BPS;
  const creatorEscrow = kind === 'bonded' ? (creator * CREATOR_ESCROW_BPS) / BPS : 0n;
  return { creatorNow: creator - creatorEscrow, creatorEscrow, interface: iface, rektBack, protocol: fee - creator - iface - rektBack };
}

export const feeOn = (amount, feeBps) => ceilDiv(amount * feeBps, BPS);

export const CURVE_EXAMPLE = Object.freeze({
  v0: 30n * NATIVE,
  t0: 1_000_000_000n * TOKEN, // = curveSupply + lpSupply: no virtual tokens beyond the real supply
  totalSupply: 1_000_000_000n * TOKEN,
  curveSupply: 800_000_000n * TOKEN,
  lpSupply: 200_000_000n * TOKEN,
  feeBps: 100n,
  migrationFee: NATIVE / 50n, // 0.02: pays the keeper that calls breakout (pool creation rent and fees)
  kind: 'bonded',
});

// A token after its Stampede: curve trading, fee routing, breakout and migration.
export class Market {
  constructor(curve, p = CURVE_EXAMPLE, { strictLp = true } = {}) {
    const { v0, t0, totalSupply, curveSupply, lpSupply, feeBps, migrationFee, kind } = { ...CURVE_EXAMPLE, ...p };
    if (curveSupply + lpSupply !== totalSupply) throw new Error('supply split must add up to total supply');
    if (strictLp && t0 > curveSupply + lpSupply) throw new Error('t0 must not exceed curveSupply + lpSupply');
    if (curve.reserve !== curve.v - v0) throw new Error('reserve must equal v - v0');
    if (curve.t + curve.sold !== t0) throw new Error('t + sold must equal t0');
    if (curve.sold > curveSupply) throw new Error('sold above curve supply');
    Object.assign(this, { curve, v0, t0, totalSupply, curveSupply, lpSupply, feeBps, migrationFee, kind });
    this.phase = curve.sold === curveSupply ? Phase.BreakingOut : Phase.Curve;
    this.fees = { creatorNow: 0n, creatorEscrow: 0n, interface: 0n, rektBack: 0n, protocol: 0n };
    this.feeTotal = 0n;
    this.flows = { paidIn: 0n, paidOut: 0n }; // native that traders sent and received (gross)
    this.migration = null;
  }

  remaining() {
    return this.curveSupply - this.curve.sold;
  }

  route(fee, hasInterface) {
    const parts = splitFee(fee, this.kind, { hasInterface });
    for (const [k, v] of Object.entries(parts)) this.fees[k] += v;
    this.feeTotal += fee;
  }

  afterBuy() {
    if (this.curve.sold === this.curveSupply) this.phase = Phase.BreakingOut;
  }

  requireCurve() {
    if (this.phase !== Phase.Curve) throw new Error(`trading closed (${this.phase})`);
  }

  // Quote for buying exactly dt tokens (clamped to what is left on the curve).
  quoteBuyExactOut(dt) {
    const tokens = dt < this.remaining() ? dt : this.remaining();
    const cost = this.curve.quoteBuy(tokens);
    const fee = feeOn(cost, this.feeBps);
    return { tokens, cost, fee, gross: cost + fee, clamped: tokens < dt };
  }

  buyExactOut(dt, maxGross, { hasInterface = true } = {}) {
    this.requireCurve();
    if (dt <= 0n) throw new Error('amount must be positive');
    const q = this.quoteBuyExactOut(dt);
    if (q.gross > maxGross) throw new Error('slippage');
    this.curve.buy(q.tokens);
    this.route(q.fee, hasInterface);
    this.flows.paidIn += q.gross;
    this.afterBuy();
    return q;
  }

  // Tokens for exactly `gross` native (fee included). Anything not needed (clamp at the boundary, rounding) is refunded.
  quoteBuyExactIn(gross) {
    let net = (gross * BPS) / (BPS + this.feeBps);
    const c = this.curve;
    let tokens = c.t - ceilDiv(c.k, c.v + net);
    if (tokens > this.remaining()) tokens = this.remaining();
    if (tokens <= 0n) return { tokens: 0n, cost: 0n, fee: 0n, gross: 0n, refund: gross, clamped: false };
    let cost = c.quoteBuy(tokens);
    let fee = feeOn(cost, this.feeBps);
    while (cost + fee > gross && tokens > 0n) {
      tokens -= 1n;
      cost = tokens > 0n ? c.quoteBuy(tokens) : 0n;
      fee = feeOn(cost, this.feeBps);
    }
    return { tokens, cost, fee, gross: cost + fee, refund: gross - cost - fee, clamped: tokens === this.remaining() };
  }

  buyExactIn(gross, minTokens, { hasInterface = true } = {}) {
    this.requireCurve();
    const q = this.quoteBuyExactIn(gross);
    if (q.tokens === 0n || q.tokens < minTokens) throw new Error('slippage');
    this.curve.buy(q.tokens);
    this.route(q.fee, hasInterface);
    this.flows.paidIn += q.gross;
    this.afterBuy();
    return q;
  }

  quoteSell(dt) {
    const out = this.curve.quoteSell(dt);
    const fee = feeOn(out, this.feeBps);
    return { out, fee, received: out - fee };
  }

  sell(dt, minReceived, { hasInterface = true } = {}) {
    this.requireCurve();
    const q = this.quoteSell(dt);
    if (q.received < minReceived) throw new Error('slippage');
    this.curve.sell(dt);
    this.route(q.fee, hasInterface);
    this.flows.paidOut += q.received;
    return q;
  }

  // Final curve price as a fraction (native base units per token base unit).
  spot() {
    return { num: this.curve.v, den: this.curve.t };
  }

  // Permissionless. Moves the reserve and the LP tokens to the DEX at the curve's final price.
  breakout(caller) {
    if (this.phase !== Phase.BreakingOut) throw new Error('not breaking out');
    const c = this.curve;
    const fee = this.migrationFee < c.reserve ? this.migrationFee : 0n;
    const lpNative = c.reserve - fee;
    const atPrice = (lpNative * c.t) / c.v; // tokens that match lpNative at the final curve price
    const lpTokens = atPrice < this.lpSupply ? atPrice : this.lpSupply;
    this.migration = {
      caller,
      keeperFee: fee,
      lpNative,
      lpTokens,
      burned: this.lpSupply - lpTokens,
      finalSpot: { num: c.v, den: c.t },
      dexPrice: { num: lpNative, den: lpTokens },
    };
    c.reserve = 0n;
    this.phase = Phase.Migrated;
    return this.migration;
  }
}

// Stampede, then a market on the resulting curve.
export function marketAfterLobby(poolNative, p = CURVE_EXAMPLE, opts) {
  const params = { ...CURVE_EXAMPLE, ...p };
  const lobby = exampleLobby({ v0: params.v0, t0: params.t0, curveSupply: params.curveSupply, feeBps: params.feeBps });
  if (poolNative > 0n) lobby.deposit('herd', poolNative, 0);
  const r = lobby.settle(60);
  const market = new Market(lobby.curve, params, opts);
  if (!r.empty) market.route(r.F, true);
  market.flows.paidIn += r.empty ? 0n : r.N + r.F;
  return { market, lobby, settle: r };
}

// Native needed to take the curve from its current state to breakout (cost before fees).
export function costToBreakout(market) {
  return market.remaining() === 0n ? 0n : market.curve.quoteBuy(market.remaining());
}
