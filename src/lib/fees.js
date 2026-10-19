// Fee model from document/HERDCALL.md §5 and §7. Percentages of trade volume.
export const FEE_SPLIT = {
  bonded: { creator: 0.5, interface: 0.1, rektBack: 0.1, protocol: 0.3 },
  unbonded: { creator: 0.3, interface: 0.1, rektBack: 0.15, protocol: 0.45 },
};

// Share of a bonded creator's fees held in escrow until breakout / vest end.
export const CREATOR_ESCROW_SHARE = 0.5;

// Split of the protocol share after the fixed operations floor.
export const PROTOCOL_SPLIT = [
  { key: 'herdVault', pct: 50, title: 'Herd Vault', note: 'Directed by ve-token votes' },
  { key: 'buildersFund', pct: 30, title: 'Builders Fund', note: 'Hackathons & milestone grants' },
  { key: 'operations', pct: 20, title: 'Operations', note: 'Team, audits, infra' },
];

// Diamond Hooves: weekly fee rebate for long-hold losses (document/HERDCALL.md §9). No randomness.
// Budget is a fixed slice of protocol revenue, so the program can never cost more than it earns.
export const DIAMOND_HOOVES = {
  revenueDivisor: 50, // budget = protocol revenue / 50 (2%)
  minHoldDays: 7,
  deadTokenDays: 30,
};

// Weekly budget and per-position rebate: shared in proportion to fees paid, never more than the fees paid.
export function diamondHoovesBudget(protocolRevenue) {
  return protocolRevenue / DIAMOND_HOOVES.revenueDivisor;
}

export function diamondHoovesRebate(feesPaid, eligibleFeesTotal, budget) {
  if (eligibleFeesTotal <= 0) return 0;
  return Math.min(feesPaid, (budget * feesPaid) / eligibleFeesTotal);
}

export const LOBBY_WINDOWS = [15, 30, 60];
export const BOND_OPTIONS = [2, 3, 5];
export const VEST_OPTIONS = [30, 60, 90];

export function formatPct(value) {
  return `${value.toFixed(2)}%`;
}

// Where a set of tokens' trading fees went, in USD (volume × fee % per token).
export function feeBreakdown(tokens, volumeKey = 'vol24hUsd') {
  const totals = { total: 0, creator: 0, interface: 0, rektBack: 0, protocol: 0 };
  for (const t of tokens) {
    const split = t.bonded ? FEE_SPLIT.bonded : FEE_SPLIT.unbonded;
    for (const [key, pct] of Object.entries(split)) {
      const usd = (t[volumeKey] * pct) / 100;
      totals[key] += usd;
      totals.total += usd;
    }
  }
  const protocolParts = PROTOCOL_SPLIT.map((p) => ({ ...p, usd: (totals.protocol * p.pct) / 100 }));
  return { ...totals, protocolParts };
}

export function creatorPayout(bonded) {
  const fee = bonded ? FEE_SPLIT.bonded.creator : FEE_SPLIT.unbonded.creator;
  const escrowed = bonded ? fee * CREATOR_ESCROW_SHARE : 0;
  return { now: fee - escrowed, escrowed };
}
