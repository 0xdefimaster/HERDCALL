// Cost model of the refundable name/ticker bond (document/HERDCALL.md §12). Amounts in SOL.
//
// A reservation lasts until the launch can no longer break out. The cheapest way to hold a name is an empty Lobby
// that is settled anyway (the curve still opens), so one cycle lasts the curve lifetime; at its deadline anyone can
// release the name, and the squatter can release and re-create in one transaction. The bond comes back every
// cycle; what is lost is the launch rent that never comes back, the transaction fees and the yield on the bond.

export const COSTS = {
  // Measured in LiteSVM: what create_launch costs the creator beyond the bond and the recoverable record rent.
  sunkPerLaunch: 0.00717788,
  // Rent of the two name/ticker records, back at release.
  recordRent: 0.0017272,
  // settle + release, one signature each.
  txPerCycle: 0.00001,
};

export const ASSUMPTIONS = {
  curveLifetimeDays: 7,
  // Yield the locked SOL could earn elsewhere (staking), per year.
  yieldPerYear: 0.07,
};

export function cyclesPerYear(a = ASSUMPTIONS) {
  return 365 / a.curveLifetimeDays;
}

/** Capital needed to reserve `names` at the same moment. */
export function grabCapital(names, bond, c = COSTS) {
  return names * (bond + c.recordRent + c.sunkPerLaunch);
}

/** What holding one name for `days` really costs (nothing of the bond is lost, only its yield). */
export function holdCost(days, bond, c = COSTS, a = ASSUMPTIONS) {
  const cycles = Math.max(1, Math.ceil(days / a.curveLifetimeDays));
  const lost = cycles * (c.sunkPerLaunch + c.txPerCycle);
  const yieldLost = (bond + c.recordRent) * a.yieldPerYear * (days / 365);
  return { cycles, lost, yieldLost, total: lost + yieldLost };
}

/** An honest creator: pays the bond up front, gets it back at breakout or when the launch fails. */
export function creatorCost(bond, daysLocked, c = COSTS, a = ASSUMPTIONS) {
  return {
    upfront: bond + c.recordRent + c.sunkPerLaunch,
    lost: c.sunkPerLaunch + (bond + c.recordRent) * a.yieldPerYear * (daysLocked / 365),
  };
}
