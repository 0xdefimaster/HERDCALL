// Prints the tables in document/HERDCALL.md §12 (name/ticker bond). Run: npm run sim:names
import { ASSUMPTIONS, COSTS, creatorCost, grabCapital, holdCost } from './name-bond.js';

const f = (v, d = 3) => v.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log();
}

const bonds = [0, 0.1, 0.25, 0.5, 1, 2];
console.log(`Assumptions: curve lifetime ${ASSUMPTIONS.curveLifetimeDays} days, yield ${(ASSUMPTIONS.yieldPerYear * 100).toFixed(0)}%/year, sunk per launch ${COSTS.sunkPerLaunch} SOL\n`);

console.log('1. Capital to reserve N names at once (SOL)');
table(['Bond', 'N = 100', 'N = 1,000', 'N = 10,000'], bonds.map((b) => [f(b, 2), ...[100, 1000, 10000].map((n) => f(grabCapital(n, b), 0))]));

console.log('2. Real cost of holding one name (SOL, bond returned)');
table(['Bond', '7 days', '30 days', '1 year', '100 names, 1 year'], bonds.map((b) => [
  f(b, 2), f(holdCost(7, b).total), f(holdCost(30, b).total), f(holdCost(365, b).total), f(100 * holdCost(365, b).total, 1),
]));

console.log('3. Honest creator (SOL)');
table(['Bond', 'Up front', 'Lost if the bond is locked 1 day', 'Lost if locked 7 days'], bonds.map((b) => [
  f(b, 2), f(creatorCost(b, 1).upfront), f(creatorCost(b, 1).lost, 4), f(creatorCost(b, 7).lost, 4),
]));
