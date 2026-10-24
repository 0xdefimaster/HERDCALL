// Tables for CURVE_SPEC §5.7 ("back to liquidity"). Run: npm run sim:reinvest
import { bestSandwich, reinvest, safeShare } from './lp-reinvest.js';

const pct = (v, d = 2) => `${(v * 100).toFixed(d)}%`;
const pool = { x: 120, y: 160_000_000 }; // pool right after an example breakout: ~120 SOL, ~160M tokens

console.log('1. Largest reinvest per call that no sandwich profits from (share of the pool\'s SOL)');
console.log('| Pool fee | Safe share |\n|---|---|');
for (const f of [0.0025, 0.005, 0.01, 0.02]) console.log(`| ${pct(f)} | ${pct(safeShare(f))} |`);

console.log('\n2. 1% pool, 120 SOL: one reinvest of p SOL');
console.log('| p | Swapped | Best sandwich profit | Attack size |\n|---|---|---|---|');
for (const p of [0.1, 0.5, 1, 2, 4, 8]) {
  const r = reinvest(pool, p, 0.01);
  const s = bestSandwich(pool, p, 0.01);
  console.log(`| ${p} SOL | ${r.swapped.toFixed(3)} SOL | ${s.profit.toFixed(5)} SOL | ${s.a.toFixed(1)} SOL |`);
}
