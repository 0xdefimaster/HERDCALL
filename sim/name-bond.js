// Cost model of name/ticker squatting (document/HERDCALL.md §12). Amounts in SOL.
//
// A name is held during the Lobby against a refundable bond. When the Lobby reaches the minimum, the curve opens,
// the name becomes permanent and the bond goes back; a cancelled Lobby releases the name (bond back too). So the
// only way to take a name for good is a Lobby that reaches the minimum. The cheapest version: the squatter fills it
// alone and sells every token back. What is lost for good is the Stampede and sell fees (minus the creator and
// interface shares the squatter gets back) and the Stampede surplus N²/(V0 + 2N) that stays in the curve
// (STAMPEDE_SPEC §4.6). The bond is only tied up for the Lobby, so it costs (almost) nothing.

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

/** Taking one name for good: the one-off loss of the minimum Lobby, the launch rent and the transactions. */
export function squatCost(minLobby, c = COSTS) {
  return squatLoss(minLobby).loss + c.sunkPerLaunch + c.txPerSquat;
}

/** Capital to take `names` at once: bond and Lobby during the Lobby; all of it comes back except the loss. */
export function squatCapital(names, minLobby, bond, c = COSTS) {
  return {
    peak: names * (bond + c.recordRent + c.sunkPerLaunch + minLobby),
    lost: names * squatCost(minLobby, c),
  };
}

/** An honest creator: bond up front, back when the curve opens or the Lobby is cancelled (after `hoursLocked`). */
export function creatorCost(bond, hoursLocked, c = COSTS, a = ASSUMPTIONS) {
  return {
    upfront: bond + c.recordRent + c.sunkPerLaunch,
    lost: c.sunkPerLaunch + (bond + c.recordRent) * a.yieldPerYear * (hoursLocked / (365 * 24)),
  };
}
