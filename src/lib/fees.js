// Fee model from document/HERDCALL.md §5 and §7. Percentages of trade volume.
export const FEE_SPLIT = {
  bonded: { creator: 0.3, interface: 0.1, rektBack: 0.15, protocol: 0.45 },
  unbonded: { creator: 0.1, interface: 0.1, rektBack: 0.35, protocol: 0.45 },
};

// Share of a bonded creator's fees held in escrow until breakout / vest end.
export const CREATOR_ESCROW_SHARE = 0.5;

// Split of the protocol share after the fixed operations floor.
export const PROTOCOL_SPLIT = [
  { key: 'herdVault', pct: 50, title: 'Herd Vault', note: 'Directed by ve-token votes' },
  { key: 'buildersFund', pct: 30, title: 'Builders Fund', note: 'Hackathons & milestone grants' },
  { key: 'operations', pct: 20, title: 'Operations', note: 'Team, audits, infra' },
];

export const LOBBY_WINDOWS = [15, 30, 60];
export const BOND_OPTIONS = [2, 3, 5];
export const VEST_OPTIONS = [30, 60, 90];

export function formatPct(value) {
  return `${value.toFixed(2)}%`;
}

export function creatorPayout(bonded) {
  const fee = bonded ? FEE_SPLIT.bonded.creator : FEE_SPLIT.unbonded.creator;
  const escrowed = bonded ? fee * CREATOR_ESCROW_SHARE : 0;
  return { now: fee - escrowed, escrowed };
}
