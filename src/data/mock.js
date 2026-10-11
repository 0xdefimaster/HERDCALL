// Example data for the UI until the on-chain program and indexer exist.

// status: 'lobby' (countdown running) | 'curve' (trading on the bonding curve)
//         | 'broke' (broke out to a DEX) | 'takeover' (community vote open)
const TOKEN_DEFAULTS = {
  bonded: false,
  bondSol: 0,
  vestDays: 0,
  launches: 1,
  trampled: 0,
  price: '—',
  clearingPrice: '—',
  marketCapUsd: 0,
  herd: 0,
  breakoutProgress: 0,
  fairScore: 50,
  ageMin: 0,
  lobbySecs: 0,
  pen: 0,
  vol24hUsd: 0,
  change24h: 0,
};

// Pool balances that aren't derivable from the board (example figures).
export const MOCK_POOLS = {
  rektBackBalanceUsd: 48200,
  rektBackPaidUsd: 12900,
  herdVaultUsd: 31600,
};

// 24h market stats per token (lobby tokens have none yet).
const MARKET = {
  MOOVE: { vol24hUsd: 28400, change24h: 22.4 },
  HOOF: { vol24hUsd: 9600, change24h: 41.7 },
  RODEO: { vol24hUsd: 51200, change24h: 68.2 },
  CUD: { vol24hUsd: 3100, change24h: -12.3 },
  MILK: { vol24hUsd: 38900, change24h: 15.9 },
  HAYDAY: { vol24hUsd: 7400, change24h: -4.8 },
  BRAND: { vol24hUsd: 142000, change24h: 9.3 },
  PEN: { vol24hUsd: 6200, change24h: 34.5 },
  STOMP: { vol24hUsd: 61800, change24h: -6.1 },
};

// Deterministic fake price path for the sparkline, ending up or down with the 24h change.
function sparkline(seed, change) {
  let h = 0;
  for (const c of seed) h = (h * 31 + c.charCodeAt(0)) % 9973;
  const points = [];
  let v = 50;
  for (let i = 0; i < 16; i += 1) {
    h = (h * 1103 + 12345) % 9973;
    v += (h / 9973 - 0.5) * 14 + change / 30;
    points.push(v);
  }
  return points;
}

export const MOCK_BOARD = [
  { symbol: 'BELL', name: 'Cowbell', description: 'More cowbell. Always more cowbell.', status: 'lobby', bonded: true, bondSol: 5, vestDays: 90, launches: 2, lobbySecs: 12, pen: 240.5, herd: 530, fairScore: 90 },
  { symbol: 'MOOVE', name: 'Moove', description: 'The cow that jumped over the moon, now on Solana.', status: 'curve', bonded: true, bondSol: 3, vestDays: 60, launches: 3, ageMin: 14, price: '0.0000063 SOL', clearingPrice: '0.0000041 SOL', marketCapUsd: 41200, herd: 688, breakoutProgress: 62, fairScore: 86 },
  { symbol: 'GRAZE', name: 'Graze', description: 'Slow and steady eats the field.', status: 'lobby', lobbySecs: 48, pen: 72.0, herd: 158, fairScore: 61 },
  { symbol: 'HOOF', name: 'Hoofprint', description: 'Leave a mark on the chain.', status: 'curve', bonded: true, bondSol: 2, vestDays: 30, ageMin: 6, price: '0.0000029 SOL', clearingPrice: '0.0000024 SOL', marketCapUsd: 18400, herd: 301, breakoutProgress: 34, fairScore: 82 },
  { symbol: 'RODEO', name: 'Rodeo', description: 'Eight seconds on the bull, eight weeks on the chart.', status: 'curve', bonded: true, bondSol: 5, vestDays: 60, launches: 4, ageMin: 42, price: '0.0000098 SOL', clearingPrice: '0.0000040 SOL', marketCapUsd: 63100, herd: 1204, breakoutProgress: 91, fairScore: 88 },
  { symbol: 'TAILS', name: 'Tail Swish', description: 'Swatting snipers since block one.', status: 'lobby', bonded: true, bondSol: 2, vestDays: 30, lobbySecs: 33, pen: 58.3, herd: 121, fairScore: 79 },
  { symbol: 'CUD', name: 'Cud Chewer', description: 'Chew on this.', status: 'curve', ageMin: 11, price: '0.0000016 SOL', clearingPrice: '0.0000019 SOL', marketCapUsd: 9700, herd: 96, breakoutProgress: 18, fairScore: 44 },
  { symbol: 'MILK', name: 'Milk Run', description: 'Fresh every morning.', status: 'curve', ageMin: 63, price: '0.0000087 SOL', clearingPrice: '0.0000039 SOL', marketCapUsd: 55800, herd: 870, breakoutProgress: 84, fairScore: 57 },
  { symbol: 'HAYDAY', name: 'Hay Day', description: 'Creator ran. The herd stayed.', status: 'takeover', ageMin: 28800, price: '0.0000034 SOL', clearingPrice: '0.0000041 SOL', marketCapUsd: 22600, herd: 1530, breakoutProgress: 100, fairScore: 63, trampled: 1 },
  { symbol: 'BRAND', name: 'Brand', description: 'Marked for the moon.', status: 'broke', bonded: true, bondSol: 3, vestDays: 60, launches: 5, ageMin: 1440, price: '0.000021 SOL', clearingPrice: '0.0000040 SOL', marketCapUsd: 412000, herd: 5210, breakoutProgress: 100, fairScore: 92 },
  { symbol: 'PEN', name: 'Bull Pen', description: 'Where the herd waits for the bell.', status: 'curve', bonded: true, bondSol: 2, vestDays: 30, ageMin: 3, price: '0.0000043 SOL', clearingPrice: '0.0000041 SOL', marketCapUsd: 12300, herd: 214, breakoutProgress: 9, fairScore: 76 },
  { symbol: 'STOMP', name: 'Stomp', description: 'Snipers get stomped.', status: 'broke', ageMin: 4320, price: '0.000012 SOL', clearingPrice: '0.0000041 SOL', marketCapUsd: 168000, herd: 3020, breakoutProgress: 100, fairScore: 71 },
].map((t) => ({ ...TOKEN_DEFAULTS, initial: t.name[0], ...MARKET[t.symbol], ...t, spark: sparkline(t.symbol, MARKET[t.symbol]?.change24h ?? 0) }));

export function findToken(symbol) {
  return MOCK_BOARD.find((t) => t.symbol === symbol?.toUpperCase());
}

export function formatUsd(n) {
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `$${(n / 1_000).toFixed(1)}K`;
  return `$${n}`;
}

export function formatAge(minutes) {
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 1440) return `${Math.floor(minutes / 60)}h`;
  return `${Math.floor(minutes / 1440)}d`;
}

export const MOCK_LOBBY = {
  windowSecs: 60,
  joins: [
    { who: '9fQe…xT2m', sol: '1.0' },
    { who: 'Bk7r…aa91', sol: '0.25' },
    { who: '4wLz…Pq0c', sol: '2.5' },
    { who: 'Hn3d…8kVs', sol: '0.5' },
  ],
};

export const MOCK_SIGNALS = [
  { label: 'Bellwether · 3 SOL bond, 60d vest', value: 'Yes', bar: 100, warn: false },
  { label: 'Creator history · 0 trampled', value: 'Clean', bar: 90, warn: false },
  { label: 'Top 10 holders', value: '18%', bar: 78, warn: false },
  { label: 'Linked-wallet clusters', value: '2 · 4.1%', bar: 60, warn: true },
  { label: 'Launch spread', value: '412 wallets', bar: 85, warn: false },
];

export const MOCK_HOLDERS = [
  { who: 'Bonding curve', pct: '38.0%', tag: 'Curve', kind: 'muted' },
  { who: 'Creator (vesting)', pct: '3.0%', tag: 'Bellwether', kind: 'trust' },
  { who: '4wLz…Pq0c', pct: '2.4%', tag: 'Launch entry', kind: 'body' },
  { who: 'Gt8m…Zr11', pct: '2.2%', tag: 'Cluster A', kind: 'warn' },
  { who: 'Rr2k…Ue7b', pct: '1.9%', tag: 'Cluster A', kind: 'warn' },
  { who: '9fQe…xT2m', pct: '1.6%', tag: 'Launch entry', kind: 'body' },
];

export const MOCK_CLAIMS = [
  { token: '$RUGBULL', what: 'Bonded creator sold on day 4 — trampled', sol: '0.52 SOL' },
  { token: '$HAYDAY', what: 'Bonded creator sold on day 19 — trampled', sol: '0.21 SOL' },
  { token: '$CUDCHEW', what: 'Unbonded, linked-wallet dump — pool cover', sol: '0.11 SOL' },
];

export const XP_RULES = [
  { what: 'Answered a call', pts: '+40' },
  { what: 'Held through day 7', pts: '+120' },
  { what: 'Came back after getting rekt', pts: '+200' },
  { what: 'Voted in a Takeover', pts: '+60' },
  { what: 'Bought a token that broke out', pts: '+80' },
];

export const MOCK_BADGES = [
  { name: 'First Call', earned: true },
  { name: 'Rekt 5x, still here', earned: true },
  { name: 'Took the Reins', earned: true },
  { name: 'More Cowbell', earned: false },
];

export const MOCK_TAKEOVER = {
  symbol: 'HAYDAY',
  endsIn: '1d 14h',
  forPct: 64,
  againstPct: 8,
  quorum: '12.4% of supply',
  proposerShare: '7.2%',
  votingPower: '0.42% of supply',
  signers: [
    { who: '4wLz…Pq0c', note: 'Top holder' },
    { who: 'Hn3d…8kVs', note: 'Community mod' },
    { who: 'Qa9t…L2xe', note: 'Artist' },
    { who: 'Zm4p…9Yuw', note: 'Holder since day 1' },
    { who: 'Ce7v…Rb30', note: 'Elected by vote' },
  ],
};
