// Example data for the UI until the on-chain program and indexer exist.
export const MOCK_TOKEN = {
  name: 'Moove',
  symbol: 'MOOVE',
  initial: 'M',
  description: 'The cow that jumped over the moon, now on Solana.',
  bondSol: 3,
  vestDays: 60,
  launches: 3,
  trampled: 0,
  price: '0.0000063 SOL',
  clearingPrice: '0.0000041 SOL',
  marketCap: '$41.2K',
  herd: 688,
  breakoutProgress: 62,
  fairScore: 86,
};

export const MOCK_LOBBY = {
  windowSecs: 30,
  startSecs: 27,
  herd: 412,
  pen: 186.4,
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
