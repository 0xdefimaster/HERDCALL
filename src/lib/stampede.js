// Display-only Stampede projection (document/STAMPEDE_SPEC.md §4 and §6). Floats, in whole SOL and tokens.
// The settlement itself uses the integer model in sim/stampede.js; this mirrors its formula for the live gauge.
export const STAMPEDE = {
  virtualSol: 30,
  virtualTokens: 1_000_000_000,
  totalSupply: 1_000_000_000,
  feeBps: 100,
};

// If the bell rang with `poolSol` in the pen: tokens the Lobby gets, and the one mcap everyone enters at.
// The curve opens at this same price.
export function projectStampede(poolSol, p = STAMPEDE) {
  if (!(poolSol > 0)) return null;
  const net = poolSol * (1 - p.feeBps / 10_000);
  const tokens = (net * p.virtualTokens) / (p.virtualSol + 2 * net);
  return { tokens, mcapSol: (poolSol / tokens) * p.totalSupply };
}

// A deposit's share of the projected allocation.
export function projectShare(depositSol, poolSol, p = STAMPEDE) {
  const proj = projectStampede(poolSol, p);
  if (!proj || !(depositSol > 0)) return 0;
  return (depositSol / poolSol) * proj.tokens;
}
