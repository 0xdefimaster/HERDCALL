// Exact quotes for on-chain launches, from the JS reference model (sim/), which the Rust core matches on the shared
// test vectors. The program recomputes everything and enforces the limits; quotes set those limits.
import { Market } from '../../sim/curve.js';
import { CurveState, settleMath } from '../../sim/stampede.js';
import { big } from './format';

const marketParams = (p, kind) => ({
  v0: big(p.v0), t0: big(p.t0), totalSupply: big(p.totalSupply), curveSupply: big(p.curveSupply),
  lpSupply: big(p.lpSupply), feeBps: BigInt(p.feeBps), migrationFee: big(p.migrationFee), kind,
});

/** The curve of a launch in the Curve phase, as the program holds it (k is stored, not recomputed). */
export function marketOf(launch) {
  const curve = new CurveState({ v: big(launch.v), t: big(launch.t), reserve: big(launch.reserve), sold: big(launch.sold) });
  curve.k = big(launch.k);
  return new Market(curve, marketParams(launch.params, launch.bonded ? 'bonded' : 'unbonded'));
}

/** Lower bound after slippage: `bps` of tolerance, rounded down. */
export const withSlippage = (amount, bps) => (amount * (10_000n - BigInt(bps))) / 10_000n;
/** Upper bound after slippage, rounded up. */
export const withSlippageUp = (amount, bps) => (amount * (10_000n + BigInt(bps)) + 9_999n) / 10_000n;

/** If the bell rang now with `total` deposited: the Lobby's tokens and the net SOL that buys them. */
export function projectLobby(launch, total = big(launch.totalDeposits)) {
  const p = launch.params;
  return settleMath(
    { v0: big(p.v0), t0: big(p.t0), curveSupply: big(p.curveSupply), feeBps: BigInt(p.feeBps), minTotal: big(p.minTotal) },
    total,
  );
}

/** A depositor's share of the projected Lobby tokens (the program's claim_tokens rounds the same way: down). */
export const lobbyShare = (deposit, total, lobbyTokens) => (total > 0n ? (deposit * lobbyTokens) / total : 0n);
