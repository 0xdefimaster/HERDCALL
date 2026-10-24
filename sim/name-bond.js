// Cost model of name/ticker squatting (document/HERDCALL.md §12). Amounts in SOL.
//
// A name is held from creation; a token on the curve keeps it with no time limit; it is released only when the
// launch is cancelled (Lobby below the minimum, or never settled). So the only way to hold a name for good is a
// Lobby that reaches the minimum. The cheapest version: the squatter fills it alone and sells every token back.
// What is lost for good is the Stampede and sell fees (minus the creator and interface shares the squatter gets
// back) and the Stampede surplus N²/(V0 + 2N) that stays in the curve (STAMPEDE_SPEC §4.6). The bond is refunded only
// at breakout or release, so a squatted name keeps it locked.

import { marketAfterLobby } from './curve.js';
import { NATIVE } from './stampede.js';

export const COSTS = {
  // Measured in LiteSVM: what create_launch costs the creator beyond the bond and the recoverable record rent.
  sunkPerLaunch: 0.00717788,
  // Rent of the two name/ticker records, back at release.
  recordRent: 0.0017272,
  // deposit, settle, claim, sell: one signature each.
  txPerSquat: 0.00002,
};

export const ASSUMPTIONS = {
  // Yield the locked SOL could earn elsewhere (staking), per year.
  yieldPerYear: 0.07,
};

const toSol = (v) => Number(v) / Number(NATIVE);

/** One-off loss of a Lobby of `minLobby` SOL filled alone, then sold back in full. Cheapest kind: unbonded. */
export function squatLoss(minLobby, kind = 'unbonded') {
  const D = BigInt(Math.round(minLobby * 1e9));
  const { market, settle } = marketAfterLobby(D, { kind });
  if (settle.empty) throw new Error('lobby below the minimum');
  const q = market.sell(settle.X, 0n, { hasInterface: true });
  // The model gives the Stampede fee an interface share too (the program does not): a little in the squatter's favour.
  const recovered = market.fees.creatorNow + market.fees.interface;
  return {
    received: toSol(q.received),
    recovered: toSol(recovered),
    surplus: toSol(market.curve.reserve),
    loss: toSol(D - q.received - recovered),
  };
}

/** Holding one name for good, over `years`: the one-off loss plus the yield of the bond that stays locked. */
export function squatCost(minLobby, bond, years, c = COSTS, a = ASSUMPTIONS) {
  const once = squatLoss(minLobby).loss + c.sunkPerLaunch + c.txPerSquat;
  return { once, total: once + (bond + c.recordRent) * a.yieldPerYear * years };
}

/** Capital tied up at once to squat `names`: the bond stays locked; the Lobby comes back minus its loss. */
export function squatCapital(names, minLobby, bond, c = COSTS) {
  return {
    peak: names * (bond + c.recordRent + c.sunkPerLaunch + minLobby),
    locked: names * (bond + c.recordRent),
    lost: names * squatCost(minLobby, bond, 0, c).once,
  };
}

/** An honest creator: bond up front, back at breakout or when the launch is cancelled. */
export function creatorCost(bond, daysLocked, c = COSTS, a = ASSUMPTIONS) {
  return {
    upfront: bond + c.recordRent + c.sunkPerLaunch,
    lost: c.sunkPerLaunch + (bond + c.recordRent) * a.yieldPerYear * (daysLocked / 365),
  };
}
