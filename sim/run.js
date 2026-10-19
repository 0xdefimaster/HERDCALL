// Prints the scenario tables used in document/STAMPEDE_SPEC.md §9. Run: npm run sim
import { EXAMPLE, NATIVE, TOKEN, exampleLobby } from './stampede.js';

const SUPPLY = Number(EXAMPLE.totalSupply);
const sol = (v) => (Number(v) / Number(NATIVE)).toFixed(2);
const mTokens = (v) => (Number(v) / Number(TOKEN) / 1e6).toFixed(1);
// Market cap in native for a price expressed as native base units per token base unit.
const mcap = (num, den) => ((Number(num) / Number(den)) * SUPPLY / Number(NATIVE)).toFixed(1);

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log('');
}

function settled(deposits, overrides) {
  const lobby = exampleLobby(overrides);
  deposits.forEach(([addr, amount]) => lobby.deposit(addr, amount, 0));
  const r = lobby.settle(60);
  for (const addr of lobby.deposits.keys()) lobby.claim(addr);
  return { lobby, r, dust: lobby.sweepDust() };
}

const equal = (D, n = 100) => Array.from({ length: n }, (_, i) => [`w${i}`, D / BigInt(n)]);

console.log(`## Demand scenarios (100 equal depositors, fee ${EXAMPLE.feeBps} bps, total supply ${mTokens(EXAMPLE.totalSupply)}M)\n`);
table(
  ['Pool D', 'Lobby tokens (M)', '% of supply', 'Lobby mcap (gross, incl. fee)', 'Curve opening mcap', 'Fee', 'Refunded', 'Token dust (units)', 'Sell-off slack'],
  [1n, 5n, 20n, 50n, 100n, 186n, 500n, 1_000n, 10_000n].map((s) => {
    const D = s * NATIVE;
    const { lobby, r, dust } = settled(equal(D));
    const c = lobby.curve;
    return [
      sol(D),
      mTokens(r.X),
      `${((Number(r.X) / SUPPLY) * 100).toFixed(1)}%`,
      mcap(D, r.X),
      mcap(c.v, c.t),
      sol(r.F),
      sol(r.refund),
      dust.token.toString(),
      sol(c.slack()),
    ];
  }),
);

console.log('## Rejected alternative: the pool buys on the plain curve (average price)\n');
{
  const v0 = Number(EXAMPLE.v0) / Number(NATIVE);
  const t0 = Number(EXAMPLE.t0) / Number(TOKEN);
  const curveSupply = Number(EXAMPLE.curveSupply) / Number(TOKEN);
  const sup = SUPPLY / Number(TOKEN);
  table(
    ['Pool D', 'A: pool mcap', 'A: curve opens at', 'A: instant gap', 'A: refunded', 'B (this spec): everyone + curve open'],
    [5, 20, 50, 100, 186].map((g) => {
      const N = g * (1 - Number(EXAMPLE.feeBps) / 1e4);
      let xa = (t0 * N) / (v0 + N);
      let refund = 0;
      let breakout = false;
      if (xa > curveSupply) {
        xa = curveSupply;
        refund = N - (v0 * curveSupply) / (t0 - curveSupply);
        breakout = true;
      }
      const avgA = ((N - refund) / xa) * sup;
      const openA = ((v0 * t0) / (t0 - xa) ** 2) * sup;
      const xb = (N * t0) / (v0 + 2 * N);
      return [g.toFixed(2), avgA.toFixed(1), breakout ? 'instant breakout' : openA.toFixed(1), breakout ? '—' : `${(openA / avgA).toFixed(1)}x`, refund.toFixed(2), ((N / xb) * sup).toFixed(1)];
    }),
  );
}

console.log('## One whale vs. many small depositors (20 native pool)\n');
{
  const deposits = [['whale', 10n * NATIVE], ...Array.from({ length: 99 }, (_, i) => [`s${i}`, (10n * NATIVE) / 99n])];
  const lobby = exampleLobby();
  deposits.forEach(([a, d]) => lobby.deposit(a, d, 0));
  lobby.settle(60);
  const w = lobby.claim('whale');
  const s = lobby.claim('s0');
  const perM = (d, q) => ((Number(d) / Number(NATIVE)) / (Number(q) / Number(TOKEN) / 1e6)).toFixed(4);
  table(
    ['Wallet', 'Deposit', 'Tokens (M)', 'Paid per 1M tokens (gross)'],
    [
      ['whale', sol(10n * NATIVE), mTokens(w.tokens), perM(10n * NATIVE, w.tokens)],
      ['small (each of 99)', sol(lobby.deposits.get('s0')), mTokens(s.tokens), perM(lobby.deposits.get('s0'), s.tokens)],
    ],
  );
}

console.log('## Wallet splitting: 100 native in 1 wallet vs. split across k wallets (plus 20 × 2 native)\n');
{
  const others = Array.from({ length: 20 }, (_, i) => [`o${i}`, 2n * NATIVE]);
  table(
    ['k wallets', 'Total tokens (base units)'],
    [1n, 10n, 100n, 1000n].map((k) => {
      const lobby = exampleLobby();
      [...Array.from({ length: Number(k) }, (_, i) => [`p${i}`, (100n * NATIVE) / k]), ...others].forEach(([a, d]) => lobby.deposit(a, d, 0));
      lobby.settle(60);
      let tok = 0n;
      for (let i = 0; i < Number(k); i += 1) tok += lobby.claim(`p${i}`).tokens;
      return [String(k), tok.toString()];
    }),
  );
}

console.log('## Right after the bell (50 native pool): what the first curve buyers pay\n');
{
  const { lobby, r } = settled(equal(50n * NATIVE));
  const c = lobby.curve;
  const lobbyPerM = (Number(r.N) / Number(r.X)) * 1e6 * Number(TOKEN) / Number(NATIVE);
  const rows = [['Lobby (everyone)', sol(r.N), mTokens(r.X), lobbyPerM.toFixed(4), '1.000']];
  for (const buyTokensM of [1n, 10n, 50n]) {
    const dt = buyTokensM * 1_000_000n * TOKEN;
    const cost = c.quoteBuy(dt);
    const perM = (Number(cost) / Number(NATIVE)) / Number(buyTokensM);
    rows.push([`First buyer, ${buyTokensM}M tokens`, sol(cost), buyTokensM.toString(), perM.toFixed(4), (perM / lobbyPerM).toFixed(3)]);
  }
  table(['Who', 'Net paid', 'Tokens (M)', 'Net per 1M tokens', 'vs. Lobby price'], rows);
}
