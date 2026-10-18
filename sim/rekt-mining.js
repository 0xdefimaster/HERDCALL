// Farm model for Rekt Mining (document/REKT_MINING.md). Specification aid, not contract code.
//
// Two layers:
//   1. Trade layer (BigInt, on the real CurveState from stampede.js): what a farmer actually pays per unit of
//      reward basis for each strategy, including fees routed back to the farmer (creator / interface share).
//   2. Season layer (floats): the farmer picks how much protocol fee to burn to maximise reward value minus cost,
//      against a fixed honest flow. Shows when farming pays and how much it dilutes honest users.
import { BPS, NATIVE, exampleLobby } from './stampede.js';

// Fee split in bps of notional (document/HERDCALL.md §5).
export const FEE = Object.freeze({
  tradeBps: 100n,
  bonded: { creator: 30n, interface: 10n, rektBack: 15n, protocol: 45n },
  unbonded: { creator: 10n, interface: 10n, rektBack: 35n, protocol: 45n },
});

const fee = (notional) => (notional * FEE.tradeBps) / BPS;
const clone = (curve) => Object.assign(Object.create(Object.getPrototypeOf(curve)), curve);
const share = (notional, bps) => (notional * bps) / BPS;

// Buy with `gross` native (fee taken from it). Returns tokens bought and the fee ledger entry.
export function buyGross(curve, gross) {
  const f = fee(gross);
  const net = gross - f;
  const dt = curve.t - (curve.k + curve.v + net - 1n) / (curve.v + net); // floor tokens for net native
  if (dt <= 0n) throw new Error('buy too small');
  const cost = curve.buy(dt); // cost <= net; the rounding remainder stays with the trader
  return { tokens: dt, paid: cost + f, notional: cost + f, fee: f };
}

// Sell dt tokens. Fee taken from the proceeds.
export function sellTokens(curve, dt) {
  const out = curve.sell(dt);
  const f = fee(out);
  return { received: out - f, notional: out, fee: f };
}

// Who gets each fee back. `creator` / `interface`: the farmer controls those wallets.
export function kickback(notional, { kind = 'bonded', creator = false, iface = false } = {}) {
  const s = FEE[kind];
  return (creator ? share(notional, s.creator) : 0n) + (iface ? share(notional, s.interface) : 0n);
}

export const protocolFee = (notional, kind = 'bonded') => share(notional, FEE[kind].protocol);

// A fresh post-Stampede curve (50 native Lobby, example parameters).
export function freshCurve(poolNative = 50n * NATIVE) {
  const lobby = exampleLobby();
  lobby.deposit('herd', poolNative, 0);
  lobby.settle(60);
  return lobby.curve;
}

export function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Honest flow: random buys and sells against the curve. `drift` < 0.5 means more selling than buying.
export function honestFlow(curve, next, { trades = 50, maxNative = 2n * NATIVE, drift = 0.5, wallets }) {
  const book = wallets ?? new Map();
  let protocol = 0n;
  for (let i = 0; i < trades; i += 1) {
    const who = `h${Math.floor(next() * 20)}`;
    const held = book.get(who) ?? 0n;
    const size = BigInt(Math.floor(next() * Number(maxNative))) + NATIVE / 100n;
    if (next() < drift || held === 0n) {
      const r = buyGross(curve, size);
      book.set(who, held + r.tokens);
      protocol += protocolFee(r.notional);
    } else {
      // Sell about `size` native worth at spot, at most the wallet's balance.
      const want = (size * curve.t) / curve.v;
      const dt = want < held ? want : held;
      const r = sellTokens(curve, dt);
      book.set(who, held - dt);
      protocol += protocolFee(r.notional);
    }
  }
  return { wallets: book, protocol };
}

// Result of one farm strategy, measured on the curve.
//   netCost    native the farmer is down after kickbacks (and capital cost, when given)
//   basis      protocol fee generated = reward basis
//   positions  [{ heldDays, loss, basis }] for multiplier rules
function ledger(legs, opts) {
  let paid = 0n;
  let received = 0n;
  let back = 0n;
  let basis = 0n;
  for (const l of legs) {
    paid += l.paid ?? 0n;
    received += l.received ?? 0n;
    back += kickback(l.notional, opts);
    basis += protocolFee(l.notional, opts.kind);
  }
  return { paid, received, kickback: back, basis, netCost: paid - received - back };
}

// S1: buy and sell back immediately, one wallet. Always a loss (fees), held 0 days.
export function roundTrip(curve, gross, opts = {}) {
  const b = buyGross(curve, gross);
  const s = sellTokens(curve, b.tokens);
  const l = ledger([b, s], opts);
  return { ...l, positions: [{ heldDays: 0, loss: s.received < b.paid, basis: l.basis }] };
}

// S2: two wallets. A buys, B buys (higher), A sells (profit), B sells (manufactured loss). Held 0 days.
export function pairTrip(curve, gross, opts = {}) {
  const a = buyGross(curve, gross);
  const b = buyGross(curve, gross);
  const as = sellTokens(curve, a.tokens);
  const bs = sellTokens(curve, b.tokens);
  const l = ledger([a, b, as, bs], opts);
  return {
    ...l,
    positions: [
      { heldDays: 0, loss: as.received < a.paid, basis: protocolFee(a.notional) + protocolFee(as.notional) },
      { heldDays: 0, loss: bs.received < b.paid, basis: protocolFee(b.notional) + protocolFee(bs.notional) },
    ],
  };
}

// S3: the pair trip, but both wallets hold through `days` of honest flow to pass a hold gate.
// The price move during the hold is reported separately (marketPnl): it is risk, not a cost of farming.
// netCost here is structural: fees − kickbacks + capital cost (`yearlyRate` on the locked capital).
export function heldPairTrip(curve, gross, next, { days = 7, flowPerDay = 20, drift = 0.5, yearlyRate = 0.07, wallets, ...opts } = {}) {
  const a = buyGross(curve, gross);
  const b = buyGross(curve, gross);
  honestFlow(curve, next, { trades: days * flowPerDay, drift, wallets });
  const as = sellTokens(curve, a.tokens);
  const bs = sellTokens(curve, b.tokens);
  const l = ledger([a, b, as, bs], opts);
  const fees = a.fee + b.fee + as.fee + bs.fee;
  const capital = BigInt(Math.round(Number(a.paid + b.paid) * yearlyRate * (days / 365)));
  return {
    ...l,
    capital,
    marketPnl: l.received - l.paid + fees,
    netCost: fees - l.kickback + capital,
    positions: [
      { heldDays: days, loss: as.received < a.paid, basis: protocolFee(a.notional) + protocolFee(as.notional) },
      { heldDays: days, loss: bs.received < b.paid, basis: protocolFee(b.notional) + protocolFee(bs.notional) },
    ],
  };
}

// S4: sandwich an honest buy that accepts `slippageBps` over its quote. Shown to separate MEV from Rekt Mining.
export function sandwich(curve, honestGross, { slippageBps = 100n, ...opts } = {}) {
  const probe = clone(curve);
  const quoted = buyGross(probe, honestGross).tokens;
  const minOut = (quoted * (BPS - slippageBps)) / BPS;
  // Largest front-run (binary search) that still lets the honest buy clear its minimum.
  let lo = 0n;
  let hi = honestGross * 50n;
  while (hi - lo > NATIVE / 1000n) {
    const mid = (lo + hi) / 2n;
    const c = clone(curve);
    buyGross(c, mid);
    if (buyGross(c, honestGross).tokens >= minOut) lo = mid;
    else hi = mid;
  }
  if (lo < NATIVE / 100n) return null;
  const f = buyGross(curve, lo);
  const h = buyGross(curve, honestGross);
  const s = sellTokens(curve, f.tokens);
  const l = ledger([f, s], opts);
  return { ...l, frontRun: lo, honestTokens: h.tokens, minOut, positions: [{ heldDays: 0, loss: s.received < f.paid, basis: l.basis }] };
}

// ---------------------------------------------------------------- multipliers

// Rekt multiplier `mRekt` for loss-closing positions, hold multiplier `mHold` for positions held >= gateDays.
// gate: true  -> the rekt multiplier also needs the hold gate (v1 proposal)
// cap: the product of multipliers never exceeds `cap`.
export function multiplier({ heldDays, loss }, { mRekt = 1, mHold = 1, gateDays = 7, gate = true, cap = 2 }) {
  const long = heldDays >= gateDays;
  let m = 1;
  if (long) m *= mHold;
  if (loss && (long || !gate)) m *= mRekt;
  return Math.min(m, cap);
}

// Honest flow summarised as shares of reward basis per position class.
export const HONEST_MIX = Object.freeze({ shortWin: 0.25, shortLoss: 0.45, longWin: 0.1, longLoss: 0.2 });

export function honestMultiplier(design, mix = HONEST_MIX) {
  return (
    mix.shortWin * multiplier({ heldDays: 0, loss: false }, design) +
    mix.shortLoss * multiplier({ heldDays: 0, loss: true }, design) +
    mix.longWin * multiplier({ heldDays: 99, loss: false }, design) +
    mix.longLoss * multiplier({ heldDays: 99, loss: true }, design)
  );
}

// Basis-weighted multiplier a strategy's positions earn.
export function strategyMultiplier(positions, design) {
  let w = 0;
  let b = 0;
  for (const p of positions) {
    const basis = Number(p.basis);
    w += basis * multiplier(p, design);
    b += basis;
  }
  return b === 0 ? 0 : w / b;
}

// ---------------------------------------------------------------- season layer
//
// Units: native, measured as protocol fee (reward basis).
//   H       honest protocol fees in the season
//   mBar    honest basis-weighted multiplier
//   mF      farmer multiplier
//   cF      farmer net cost per unit of basis (from the trade layer)
//   pool    'fixed': value = budgetValue (B_t × $HERD price), independent of fees
//           'capped': value = min(budgetValue, alpha × season protocol revenue)
//   dh      Diamond Hooves: budget = revenue / 50, shared pro-rata over eligible basis.
//           ellH = honest eligible share of basis; farmerEligible = farmer's eligible share.
export function seasonPayoff(x, p) {
  const { H, mBar, mF, cF, pool = 'capped', alpha = 0.25, budgetValue = Infinity, dh = null } = p;
  const revenue = H + x;
  const value = pool === 'fixed' ? budgetValue : Math.min(budgetValue, alpha * revenue);
  const honestPts = mBar * H;
  const farmPts = mF * x;
  const farmReward = honestPts + farmPts === 0 ? 0 : (value * farmPts) / (honestPts + farmPts);
  const honestReward = value - farmReward;
  let farmDh = 0;
  if (dh) {
    const eligibleF = x * dh.farmerEligible;
    const eligibleH = H * dh.ellH;
    const budget = revenue / 50;
    farmDh = eligibleH + eligibleF === 0 ? 0 : Math.min(eligibleF / 0.45, (budget * eligibleF) / (eligibleH + eligibleF));
  }
  return { value, farmReward, farmDh, honestReward, profit: farmReward + farmDh - cF * x };
}

// Best farm size on a log grid plus the closed form where it exists.
export function bestFarm(p, { steps = 4000 } = {}) {
  let best = { x: 0, profit: 0 };
  for (let i = 1; i <= steps; i += 1) {
    const x = p.H * 10 ** (-3 + (6 * i) / steps); // from H/1000 up to 1000·H
    const r = seasonPayoff(x, p);
    if (r.profit > best.profit) best = { x, profit: r.profit };
  }
  const base = seasonPayoff(0, p).honestReward;
  const withFarm = seasonPayoff(best.x, p).honestReward;
  return { ...best, honestRewardRatio: base === 0 ? 1 : withFarm / base };
}

// Fixed pool closed form: farming pays iff V·mF > cF·mBar·H; x* = (sqrt(V·a·b/c) − a)/b with a = mBar·H, b = mF.
export function fixedPoolOptimum({ H, mBar, mF, cF, budgetValue: V }) {
  const a = mBar * H;
  const b = mF;
  if (V * b <= cF * a) return { x: 0, profit: 0 };
  const x = (Math.sqrt((V * a * b) / cF) - a) / b;
  return { x, profit: (V * b * x) / (a + b * x) - cF * x };
}

// Capped pool safety condition (document/REKT_MINING.md §4). Margin >= 0 means no farm size pays:
//   alpha · max(1, mF / mBar) + max(1, farmerEligible / ellH) / 50 <= cF
export function cappedSafetyMargin({ alpha, mF, mBar, cF, dh = null }) {
  const reward = alpha * Math.max(1, mF / mBar);
  const dhTerm = dh ? Math.max(1, dh.farmerEligible / dh.ellH) / 50 : 0;
  return cF - reward - dhTerm;
}

