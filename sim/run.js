// Prints the scenario tables used in document/STAMPEDE_SPEC.md §9. Run: npm run sim
import { EXAMPLE, NATIVE, TOKEN, exampleLobby, makeCurve } from './stampede.js';

const curve = makeCurve(EXAMPLE.curve);
const fmtNative = (v) => (Number(v) / Number(NATIVE)).toFixed(3);
const fmtTokensM = (v) => (Number(v) / Number(TOKEN) / 1e6).toFixed(2);
// Price shown as native per 1M tokens.
const perMillion = (nativeAmount, tokens) => (tokens === 0n ? '—' : ((Number(nativeAmount) / Number(NATIVE)) / (Number(tokens) / Number(TOKEN) / 1e6)).toFixed(4));
const spotPerMillion = (x) => {
  const { num, den } = curve.spotPrice(x);
  return ((Number(num) / Number(den)) * Number(TOKEN) * 1e6 / Number(NATIVE)).toFixed(4);
};

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log('');
}

function simulate(deposits, overrides) {
  const lobby = exampleLobby(overrides);
  deposits.forEach(([addr, amount]) => lobby.deposit(addr, amount, 0));
  const r = lobby.settle(60);
  let refunds = 0n;
  let tokens = 0n;
  for (const addr of lobby.deposits.keys()) {
    const c = lobby.claim(addr);
    refunds += c.refund;
    tokens += c.tokens;
  }
  return { lobby, r, refunds, tokens, dust: lobby.sweepDust() };
}

console.log('## Lobby capacity: C(S_L) for different Lobby slices (example curve)\n');
table(
  ['S_L (% of curve supply)', 'S_L (M tokens)', 'C(S_L) net', 'avg price / 1M', 'spot after / 1M'],
  [10n, 20n, 30n, 50n, 100n].map((pct) => {
    const sl = (EXAMPLE.curveSupply * pct) / 100n;
    const c = curve.costOf(sl);
    return [`${pct}%`, fmtTokensM(sl), fmtNative(c), perMillion(c, sl), spotPerMillion(sl)];
  }),
);

console.log(`## Demand scenarios (S_L = ${fmtTokensM(EXAMPLE.lobbySupply)}M tokens, fee ${EXAMPLE.feeBps} bps, 100 equal depositors)\n`);
const demands = [0n, NATIVE / 20n, NATIVE, 3n * NATIVE, 4_926_000_000n, 10n * NATIVE, 186_400_000_000n, 1_000n * NATIVE];
table(
  ['Total D', 'Case', 'X (M tokens)', '% of S_L', 'U → curve', 'F fee', 'Refunded', 'Refund %', 'Clearing price / 1M (gross)', 'Spot after / 1M', 'Dust (token units / native units)'],
  demands.map((D) => {
    const n = 100n;
    const each = D / n;
    const deposits = each >= EXAMPLE.minDeposit ? Array.from({ length: 100 }, (_, i) => [`w${i}`, each]) : [];
    if (deposits.length === 0 && D > 0n) deposits.push(['solo', D < EXAMPLE.minDeposit ? EXAMPLE.minDeposit : D]);
    const s = simulate(deposits);
    const total = s.lobby.total;
    const caseName = s.r.empty ? 'empty' : s.r.oversubscribed ? 'D ≥ C(S_L)' : 'D < C(S_L)';
    return [
      fmtNative(total),
      caseName,
      fmtTokensM(s.r.X),
      s.r.empty ? '0%' : `${((Number(s.r.X) / Number(EXAMPLE.lobbySupply)) * 100).toFixed(1)}%`,
      fmtNative(s.r.U),
      fmtNative(s.r.F),
      fmtNative(s.refunds),
      total === 0n ? '—' : `${((Number(s.refunds) / Number(total)) * 100).toFixed(1)}%`,
      perMillion(s.r.G, s.r.X),
      spotPerMillion(s.r.X),
      `${s.dust.token} / ${s.dust.native}`,
    ];
  }),
);

console.log('## One whale vs. many small depositors (oversubscribed, 20 SOL total)\n');
{
  const deposits = [['whale', 10n * NATIVE], ...Array.from({ length: 99 }, (_, i) => [`s${i}`, (10n * NATIVE) / 99n])];
  const s = simulate(deposits);
  const whale = s.lobby.deposits.get('whale');
  const small = s.lobby.deposits.get('s0');
  const cw = { d: whale };
  const cs = { d: small };
  // Re-derive per-address results from the formulas (claims already executed above).
  const share = (d) => ({ tokens: (d * s.r.X) / s.lobby.total, refund: (d * s.r.E) / s.lobby.total });
  Object.assign(cw, share(whale));
  Object.assign(cs, share(small));
  table(
    ['Wallet', 'Deposit', 'Tokens (M)', 'Refund', 'Paid per 1M (gross)'],
    [
      ['whale', fmtNative(cw.d), fmtTokensM(cw.tokens), fmtNative(cw.refund), perMillion(cw.d - cw.refund, cw.tokens)],
      ['small (each of 99)', fmtNative(cs.d), fmtTokensM(cs.tokens), fmtNative(cs.refund), perMillion(cs.d - cs.refund, cs.tokens)],
    ],
  );
}

console.log('## Wallet splitting: one 100 SOL deposit vs. the same split across k wallets (plus 20 × 2 SOL)\n');
{
  const others = Array.from({ length: 20 }, (_, i) => [`o${i}`, 2n * NATIVE]);
  const rows = [1n, 10n, 100n, 1000n].map((k) => {
    const parts = Array.from({ length: Number(k) }, (_, i) => [`p${i}`, (100n * NATIVE) / k]);
    const lobby = exampleLobby();
    [...parts, ...others].forEach(([a, d]) => lobby.deposit(a, d, 0));
    lobby.settle(60);
    let tok = 0n;
    let ref = 0n;
    for (let i = 0; i < Number(k); i += 1) {
      const c = lobby.claim(`p${i}`);
      tok += c.tokens;
      ref += c.refund;
    }
    return [String(k), tok.toString(), ref.toString()];
  });
  table(['k wallets', 'Total tokens (base units)', 'Total refund (native base units)'], rows);
}
