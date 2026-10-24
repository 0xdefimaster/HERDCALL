// Prints the tables in document/HERDCALL.md §12 (name/ticker squatting). Run: npm run sim:names
import { COSTS, creatorCost, squatCapital, squatCost, squatLoss } from './name-bond.js';

const f = (v, d = 3) => v.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });

function table(headers, rows) {
  console.log(`| ${headers.join(' | ')} |`);
  console.log(`|${headers.map(() => '---').join('|')}|`);
  for (const r of rows) console.log(`| ${r.join(' | ')} |`);
  console.log();
}

const BOND = 0.25;
const lobbies = [0.1, 0.5, 1, 2, 3, 5];
console.log(`Assumptions: bond ${BOND} SOL (back when the curve opens), sunk per launch ${COSTS.sunkPerLaunch} SOL\n`);

console.log('1. Taking one name for good with a minimum Lobby filled alone and sold back (SOL)');
table(['Min Lobby', 'Received back', 'Fee shares back', 'Surplus left in the curve', 'Lost for good'], lobbies.map((d) => {
  const l = squatLoss(d);
  return [f(d, 1), f(l.received), f(l.recovered), f(l.surplus), f(squatCost(d))];
}));

console.log('2. Taking 1,000 names (SOL)');
table(['Min Lobby', 'Capital needed at once', 'Lost for good'], lobbies.map((d) => {
  const c = squatCapital(1000, d, BOND);
  return [f(d, 1), f(c.peak, 0), f(c.lost, 0)];
}));

console.log('3. Honest creator (SOL)');
table(['Bond', 'Up front', 'Lost (bond back after the Lobby)'], [0.1, 0.25, 0.5].map((b) => [
  f(b, 2), f(creatorCost(b, 1).upfront), f(creatorCost(b, 1).lost, 4),
]));
