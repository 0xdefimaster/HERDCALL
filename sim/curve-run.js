// Prints the tables used in document/CURVE_SPEC.md §8. Run: npm run sim:curve
import { NATIVE, TOKEN } from './stampede.js';
import { CURVE_EXAMPLE, costToBreakout, feeOn, marketAfterLobby, splitFee } from './curve.js';

const BIG = 10n ** 30n;
const sol = (v, d = 1) => (Number(v) / Number(NATIVE)).toFixed(d);
const mTok = (v) => (Number(v) / Number(TOKEN) / 1e6).toFixed(1);
const SUPPLY = Number(CURVE_EXAMPLE.totalSupply);
const mcap = (num, den) => ((Number(num) / Number(den)) * SUPPLY) / Number(NATIVE);
const pct = (v, d = 1) => `${(v * 100).toFixed(d)}%`;

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log('');
}

const LOBBIES = [0n, 5n, 20n, 50n, 100n, 186n, 500n];

console.log('## 1. From the bell to breakout (example parameters, T0 = 1B, curve 800M, LP 200M)\n');
table(
  ['Lobby pool', 'Curve opens at (mcap)', 'Native to breakout (before fees)', 'Breakout reserve', 'Breakout mcap', 'Open → breakout', 'LP native', 'LP tokens (M)', 'Burned (M)'],
  LOBBIES.map((d) => {
    const { market: m } = marketAfterLobby(d * NATIVE);
    const open = mcap(m.curve.v, m.curve.t);
    const need = costToBreakout(m);
    m.buyExactOut(m.remaining(), BIG);
    const end = mcap(m.curve.v, m.curve.t);
    const r = m.breakout('keeper');
    return [sol(d * NATIVE, 0), sol(BigInt(Math.round(open * 1e9))), sol(need), sol(r.lpNative + r.keeperFee), end.toFixed(0), `${(end / open).toFixed(1)}x`, sol(r.lpNative), mTok(r.lpTokens), mTok(r.burned)];
  }),
);

console.log('## 2. LP sizing: T0 = curve + LP (this spec) vs. T0 above it (pump-like 1.073B)\n');
table(
  ['Lobby pool', 'T0 = 1B: burned (M)', 'T0 = 1B: DEX vs. curve price', 'T0 = 1.073B: burned (M)', 'T0 = 1.073B: DEX vs. curve price'],
  [0n, 20n, 50n, 186n, 500n].map((d) => {
    const run = (p, opts) => {
      const { market: m } = marketAfterLobby(d * NATIVE, p, opts);
      m.buyExactOut(m.remaining(), BIG);
      const r = m.breakout('k');
      const ratio = (Number(r.dexPrice.num) / Number(r.dexPrice.den)) / (Number(r.finalSpot.num) / Number(r.finalSpot.den));
      return [mTok(r.burned), ratio - 1 < 1e-9 ? 'equal' : `+${pct(ratio - 1, 2)}`];
    };
    return [sol(d * NATIVE, 0), ...run({}), ...run({ t0: 1_073_000_000n * TOKEN }, { strictLp: false })];
  }),
);

console.log('## 3. Where the 1% goes: per 100 native of curve volume\n');
{
  const fee = feeOn(100n * NATIVE, 100n);
  const rows = [
    ['Bonded, via an interface', splitFee(fee, 'bonded')],
    ['Bonded, direct', splitFee(fee, 'bonded', { hasInterface: false })],
    ['Unbonded, via an interface', splitFee(fee, 'unbonded')],
  ].map(([name, p]) => [name, sol(p.creatorNow, 3), sol(p.creatorEscrow, 3), sol(p.interface, 3), sol(p.rektBack, 3), sol(p.protocol, 3)]);
  table(['Case', 'Creator now', 'Creator escrow', 'Interface', 'Rekt-back pool', 'Protocol'], rows);
}

console.log('## 4. Sandwich on the curve (20 native Lobby, victim buys exact-in, attacker pays the 1% fee both ways)\n');
{
  const rows = [];
  for (const victim of [1n, 5n, 20n]) {
    for (const slipBps of [50n, 100n, 300n, 500n]) {
      const base = marketAfterLobby(20n * NATIVE).market;
      const quoted = base.quoteBuyExactIn(victim * NATIVE).tokens;
      const minTokens = (quoted * (10_000n - slipBps)) / 10_000n;
      // Largest front-run that still lets the victim's minimum pass.
      let lo = 0n;
      let hi = 400n * NATIVE;
      while (hi - lo > NATIVE / 1000n) {
        const mid = (lo + hi) / 2n;
        const m = marketAfterLobby(20n * NATIVE).market;
        m.buyExactIn(mid, 1n);
        if (m.quoteBuyExactIn(victim * NATIVE).tokens >= minTokens) lo = mid;
        else hi = mid;
      }
      let profit = 0n;
      if (lo > NATIVE / 1000n) {
        const m = marketAfterLobby(20n * NATIVE).market;
        const front = m.buyExactIn(lo, 1n);
        m.buyExactIn(victim * NATIVE, minTokens);
        const back = m.sell(front.tokens, 0n);
        profit = back.received - front.gross;
      }
      rows.push([sol(victim * NATIVE, 0), pct(Number(slipBps) / 1e4), sol(lo, 2), sol(profit, 4), profit > 0n ? pct(Number(profit) / Number(victim * NATIVE), 2) : 'not profitable']);
    }
  }
  table(['Victim buy', 'Victim slippage', 'Front-run size', 'Attacker profit', 'of the victim buy'], rows);
}

console.log('## 5. Lobby price vs. breakout price\n');
table(
  ['Lobby pool', 'Lobby tokens (M)', 'Lobby price (mcap)', 'Breakout mcap', 'Lobby price → breakout'],
  LOBBIES.filter((d) => d > 0n).map((d) => {
    const { market: m, settle } = marketAfterLobby(d * NATIVE);
    const lobbyMcap = mcap(d * NATIVE, settle.X);
    m.buyExactOut(m.remaining(), BIG);
    const end = mcap(m.curve.v, m.curve.t);
    return [sol(d * NATIVE, 0), mTok(settle.X), lobbyMcap.toFixed(1), end.toFixed(0), `${(end / lobbyMcap).toFixed(1)}x`];
  }),
);
