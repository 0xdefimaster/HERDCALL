// Prints the tables used in document/REKT_MINING.md §5. Run: npm run sim:rekt
import { NATIVE } from './stampede.js';
import {
  HONEST_MIX,
  HerdPool,
  POLICY,
  RektTreasury,
  bestFarm,
  cappedSafetyMargin,
  freshCurve,
  heldPairTrip,
  honestFlow,
  honestMultiplier,
  multiplier,
  pairTrip,
  policyPool,
  rng,
  roundTrip,
  runPurchase,
  sandwich,
  strategyMultiplier,
} from './rekt-mining.js';

const sol = (v) => Number(v) / Number(NATIVE);
const f = (v, d = 2) => v.toFixed(d);
const pct = (v) => `${(v * 100).toFixed(0)}%`;

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log('');
}

// Post-Stampede market with some honest history, so sells have someone to sell to.
function market(next) {
  const curve = freshCurve();
  const wallets = honestFlow(curve, next, { trades: 200, drift: 0.7 }).wallets;
  return { curve, wallets };
}

const DESIGNS = {
  flat: { label: 'Flat (no multipliers)', mRekt: 1, mHold: 1, gate: true, cap: 1 },
  naive: { label: 'Rekt 2x, no hold gate', mRekt: 2, mHold: 1.5, gate: false, cap: 3 },
  gated: { label: 'Rekt 2x, 7-day gate', mRekt: 2, mHold: 1.5, gate: true, cap: 3 },
  proposal: { label: 'Proposal: rekt 1.5x, hold 1.25x, gate, cap 2', mRekt: 1.5, mHold: 1.25, gate: true, cap: 2 },
};

// ------------------------------------------------------------ 1. trade layer
const RUNS = 300;
const GROSS = 5n * NATIVE;
const strategies = [
  ['S1 round trip', (m) => roundTrip(m.curve, GROSS)],
  ['S1 round trip, own creator + interface', (m) => roundTrip(m.curve, GROSS, { creator: true, iface: true })],
  ['S2 two-wallet trip', (m) => pairTrip(m.curve, GROSS)],
  ['S3 two-wallet, held 7 days', (m, next) => heldPairTrip(m.curve, GROSS, next, { wallets: m.wallets })],
  ['S3 held 7 days, own creator + interface', (m, next) => heldPairTrip(m.curve, GROSS, next, { wallets: m.wallets, creator: true, iface: true })],
  ['S4 sandwich (1% slippage)', (m) => sandwich(m.curve, GROSS)],
];

const measured = {};
const rows1 = strategies.map(([name, run], si) => {
  const next = rng(100 + si);
  let cost = 0;
  let basis = 0;
  let losses = 0;
  let positions = 0;
  const pnl = [];
  const allPositions = [];
  for (let i = 0; i < RUNS; i += 1) {
    const r = run(market(next), next);
    if (!r) continue;
    cost += sol(r.netCost);
    basis += sol(r.basis);
    for (const p of r.positions) {
      positions += 1;
      if (p.loss) losses += 1;
      allPositions.push(p);
    }
    if (r.marketPnl !== undefined) pnl.push(sol(r.marketPnl));
  }
  measured[name] = { cF: cost / basis, positions: allPositions };
  const mean = pnl.length ? pnl.reduce((a, b) => a + b, 0) / pnl.length : null;
  const sd = pnl.length ? Math.sqrt(pnl.reduce((a, x) => a + (x - mean) ** 2, 0) / pnl.length) : null;
  return [name, f(cost / basis), pct(losses / positions), mean === null ? '—' : `${f(mean)} ± ${f(sd)}`];
});

console.log(`## 1. Farmer cost per 1 SOL of protocol fee (reward basis), ${sol(GROSS)} SOL legs, ${RUNS} runs on the real curve\n`);
table(['Strategy', 'Net cost per 1 SOL basis', 'Positions closed at a loss', 'Price PnL during hold (SOL, mean ± sd)'], rows1);

// ------------------------------------------------------------ 2. multipliers
const S1 = measured['S1 round trip'].positions;
const S3 = measured['S3 two-wallet, held 7 days'].positions;
console.log('## 2. Multipliers: honest users vs. farmers (basis-weighted)\n');
console.log(`Honest mix: ${Object.entries(HONEST_MIX).map(([k, v]) => `${k} ${pct(v)}`).join(', ')}\n`);
table(
  ['Design', 'Honest m̄', 'Farmer S1 (round trip)', 'Farmer S3 (held pair)', 'Worst case (cap) / m̄'],
  Object.values(DESIGNS).map((d) => {
    const mBar = honestMultiplier(d);
    return [d.label, f(mBar), f(strategyMultiplier(S1, d)), f(strategyMultiplier(S3, d)), f(d.cap / mBar)];
  }),
);

// ------------------------------------------------------------ season layer
const H = 450; // honest protocol fees in the season: 100k SOL of honest volume × 0.45%
const proposal = DESIGNS.proposal;
const mBar = honestMultiplier(proposal);
const cheapest = measured['S1 round trip, own creator + interface'].cF;
const dhHonest = HONEST_MIX.longLoss; // eligible for Diamond Hooves: held 7+ days and sold at a loss

console.log(`## 3. Fixed budget (value V = B_t × $HERD price), proposal design, H = ${H} SOL honest protocol fees\n`);
console.log(`Farmer: cheapest cost ${f(cheapest)} per basis (own creator + interface), multiplier at the cap (${proposal.cap}).\n`);
table(
  ['V / H', 'V (SOL)', 'Farmer burns (× H)', 'Farmer profit (SOL)', 'Honest users keep'],
  [0.1, 0.5, 1, 2, 5, 10].map((ratio) => {
    const p = { pool: 'fixed', H, mBar, mF: proposal.cap, cF: cheapest, budgetValue: ratio * H };
    const b = bestFarm(p);
    return [f(ratio, 1), f(ratio * H, 0), f(b.x / H), f(b.profit, 1), pct(b.honestRewardRatio)];
  }),
);

console.log(`## 4. Capped pool (value <= alpha × season protocol revenue), worst-case farmer, H = ${H} SOL\n`);
table(
  ['Design', 'alpha', 'Safety margin', 'Best farm profit (SOL)', 'Honest keep if someone burns 1×H anyway'],
  ['naive', 'gated', 'proposal'].flatMap((key) =>
    [0.1, 0.25, 0.5, 1].map((alpha) => {
      const d = DESIGNS[key];
      const m = honestMultiplier(d);
      const p = { pool: 'capped', H, mBar: m, mF: d.cap, cF: cheapest, alpha, dh: { ellH: dhHonest, farmerEligible: 1 } };
      const b = bestFarm(p);
      const base = alpha * H;
      const forced = (alpha * 2 * H * m * H) / (m * H + d.cap * H);
      return [d.label, f(alpha), f(cappedSafetyMargin(p)), f(b.profit, 1), pct(forced / base)];
    }),
  ),
);

console.log('## 5. Rekt multiplier sweep (7-day gate, hold 1.25x, alpha 0.25, worst-case farmer)\n');
table(
  ['Rekt x', 'Cap', 'Honest m̄', 'Cap / m̄', 'Safety margin', 'Honest long-loss bonus vs. flat'],
  [1, 1.25, 1.5, 2, 3].flatMap((mRekt) =>
    [2, 3].map((cap) => {
      const d = { mRekt, mHold: 1.25, gate: true, cap };
      const m = honestMultiplier(d);
      const p = { alpha: 0.25, mF: cap, mBar: m, cF: cheapest, dh: { ellH: dhHonest, farmerEligible: 1 } };
      const longLoss = multiplier({ heldDays: 99, loss: true }, d) / m; // reward per basis vs. average
      return [f(mRekt), f(cap, 0), f(m), f(cap / m), f(cappedSafetyMargin(p)), `${f(longLoss)}x`];
    }),
  ),
);


// ------------------------------------------------------------ v0.1 policy re-runs
const policyDesign = { mRekt: POLICY.mRekt, mHold: POLICY.mHold, gate: POLICY.gate, gateDays: POLICY.gateDays, cap: POLICY.cap };
const pm = honestMultiplier(policyDesign);
const OPS = 100; // example operations floor per season, in native
const worst = (H, extra = {}) => ({ pool: 'policy', H, mBar: pm, mF: POLICY.cap, cF: cheapest, alpha: POLICY.alpha, opsFloor: OPS, dh: { ellH: dhHonest, farmerEligible: 1 }, ...extra });

console.log(`## 6. v0.1 policy across season revenue (alpha ${POLICY.alpha}, ops floor ${OPS} SOL, worst-case farmer)\n`);
table(
  ['Honest volume (SOL)', 'Protocol revenue H', 'Pool (SOL)', 'Pool / revenue', 'Model: best reward-driven farm profit', 'Honest keep if someone burns 1×H'],
  [10_000, 50_000, 100_000, 1_000_000, 10_000_000].map((vol) => {
    const H = vol * 0.0045;
    const p = worst(H);
    const pool = policyPool(H, { opsFloor: OPS });
    const forcedPool = policyPool(2 * H, { opsFloor: OPS });
    const forcedHonest = forcedPool === 0 ? 0 : (forcedPool * pm * H) / (pm * H + POLICY.cap * H);
    return [vol.toLocaleString('en-US'), f(H, 0), f(pool, 1), pct(pool / H), f(bestFarm(p).profit, 1), pool === 0 ? '—' : pct(forcedHonest / pool)];
  }),
);

console.log('## 7. $HERD price scenarios: rejected fixed token budget vs. v0.1 policy (H = 450 SOL)\n');
{
  const H = 450;
  const pool = policyPool(H, { opsFloor: OPS });
  const basePrice = 0.0001; // native per $HERD at 1x
  const B = pool / basePrice; // fixed budget sized to match the policy pool at 1x
  table(
    ['$HERD price', 'Fixed B_t: pool value', 'Fixed: model farm profit', 'Fixed: honest keep', 'Policy: pool value', 'Policy: model farm profit', 'Policy: $HERD bought (M), 28 chunks, no arbitrage'],
    [0.1, 0.5, 1, 2, 5, 10].map((mult) => {
      const price = basePrice * mult;
      const fixed = bestFarm({ pool: 'fixed', H, mBar: pm, mF: POLICY.cap, cF: cheapest, budgetValue: B * price, dh: { ellH: dhHonest, farmerEligible: 1 } });
      const pol = bestFarm(worst(H));
      const tokens = runPurchase(new HerdPool({ sol: 2000, herd: 2000 / price }), { budget: pool, chunks: 28 }).tokens;
      return [`${f(mult, 1)}x`, f(B * price, 0), f(fixed.profit, 1), pct(fixed.honestRewardRatio), f(pool, 1), f(pol.profit, 1), f(tokens / 1e6, 2)];
    }),
  );
}

console.log('## 8. Controlled purchase: total premium vs. per-trade impact (pool fee 0.25%, limit 0.5% per trade)\n');
console.log('Total premium: average execution price over the starting spot, fee excluded. rho: share of each chunk\'s impact that arbitrage removes before the next chunk.\n');
{
  const L = 1000;
  const cell = (ratio, chunks, rho) => {
    const r = runPurchase(new HerdPool({ sol: L, herd: L * 10_000 }), { budget: ratio * L, chunks, rho });
    if (r.halted && r.spent === 0) return 'stopped, 100% unspent';
    const note = r.halted ? ` (stopped, ${pct(r.leftover / (ratio * L))} unspent)` : '';
    return `${pct2(r.totalPremium)} / max ${pct2(r.maxImpact)}${note}`;
  };
  table(
    ['Pool / liquidity', '1 trade', '28 chunks, rho 0 (no arbitrage)', '28 chunks, rho 0.5', '28 chunks, rho 1', '100 chunks, rho 0'],
    [0.01, 0.05, 0.1, 0.2, 0.5].map((ratio) => [f(ratio), cell(ratio, 1, 0), cell(ratio, 28, 0), cell(ratio, 28, 0.5), cell(ratio, 28, 1), cell(ratio, 100, 0)]),
  );
}

console.log('## 9. Front-running each chunk (pool / liquidity 0.05, ceiling 0.5% per trade)\n');
console.log('Slippage: how far below the pre-trade quote a chunk may fill. That gap is what a front-runner can use.\n');
table(
  ['Chunks', 'Expected impact per chunk', 'Pool fee', 'Slippage', 'Front-runner profit (% of budget)', 'Max executed impact', 'Total premium'],
  [20, 100].flatMap((chunks) =>
    [0, 25].flatMap((feeBps) =>
      [0.005, 0.001].map((slippage) => {
        const pool = new HerdPool({ sol: 1000, herd: 10_000_000, feeBps });
        const est = pool.impact(50 / chunks);
        const r = runPurchase(pool, { budget: 50, chunks, slippage, attacker: true });
        return [String(chunks), pct2(est), `${feeBps / 100}%`, pct2(slippage), pct2(r.attackerProfit / 50), pct2(r.maxImpact), pct2(r.totalPremium)];
      }),
    ),
  ),
);

console.log('## 10. Liquidity withdrawn mid-programme (pool / liquidity 0.1, 28 chunks, 50% of liquidity leaves before chunk 10)\n');
{
  const r = runPurchase(new HerdPool({ sol: 1000, herd: 10_000_000 }), { budget: 100, chunks: 28, onChunk: (p, i) => i === 10 && p.withdraw(0.5) });
  table(['Chunks bought', 'Stopped because', 'SOL spent', 'SOL unspent (owed to the season in SOL)'], [[String(r.halted.at), r.halted.reason, f(r.spent, 1), f(r.leftover, 1)]]);
}

console.log(`## 11. Treasury ledgers over four seasons (alpha ${POLICY.alpha}, ops floor ${OPS} SOL, claim window 2 seasons)\n`);
{
  const tr = new RektTreasury({ opsFloor: OPS, claimWindow: 2 });
  const points = new Map([['a', 1], ['b', 1], ['c', 2]]);
  const plan = [
    { revenue: 900, buyShare: 0.6, claimers: ['a', 'c'] },
    { revenue: 450, buyShare: 1, claimers: ['a', 'b', 'c'] },
    { revenue: 1200, buyShare: 1, claimers: ['c'] },
    { revenue: 300, buyShare: 1, claimers: ['a', 'b', 'c'] },
  ];
  const rows = plan.map((p, i) => {
    const t = i + 1;
    if (t > 2) tr.expire(t - 2, t);
    const s = tr.close(t, { revenue: p.revenue, points });
    const buy = s.solToBuy * p.buyShare;
    tr.purchase(t, buy, buy * 10_000);
    tr.endPurchase(t, t);
    p.claimers.forEach((c) => tr.claim(t, c, t));
    return [String(t), f(p.revenue, 0), f(s.allocation, 1), pct(s.allocation / p.revenue), f(s.carryUsed, 1), f(s.spent, 1), f(s.solOwed, 1), f(tr.distributedCost(t), 1), f(tr.released, 1)];
  });
  table(['Season', 'Eligible revenue', 'New allocation', 'of revenue', 'Carry used (at cost)', 'SOL spent on $HERD', 'SOL owed (unspent)', 'Distributed at cost', 'Released to protocol (cumulative)'], rows);
}

function pct2(v) {
  return `${(v * 100).toFixed(2)}%`;
}
