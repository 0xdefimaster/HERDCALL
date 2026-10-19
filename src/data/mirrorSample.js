// Sample run for the Mirror page ("Try sample data"). Made-up wallets and token; clearly labelled in the UI.
import { analyzeMirror, parseTrades } from '../lib/mirror.js';
import { curveBuyTx } from '../lib/mirrorFixture.js';

export const SAMPLE_MINT = 'SampLeMint1111111111111111111111111111111111';
export const SAMPLE_WALLET = 'YouSampLe1111111111111111111111111111111111';
export const SAMPLE_TICKER = 'SAMPLE';

const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function fakeAddress(seed) {
  let x = seed * 2654435761;
  let out = '';
  for (let i = 0; i < 44; i += 1) {
    x = (x * 1103515245 + 12345) % 2147483648;
    out += ALPHABET[x % ALPHABET.length];
  }
  return out;
}

// Creator buys in the launch slot, twelve wallets follow in the first two slots, the user arrives at slot 6.
const BUYS = [
  [0, 1, 1.5],
  ...[0.8, 2.5, 1.2, 3, 0.6, 2, 1.1].map((sol, i) => [0, 2 + i, sol]),
  ...[1.4, 0.9, 2.2, 0.7, 1.8].map((sol, i) => [1, 20 + i, sol]),
  [3, 40, 0.5],
  [6, 'you', 1],
];

export function sampleMirror() {
  let curve = { v: 30, t: 1_073_000_000 };
  const launch = 300_000_000;
  const trades = BUYS.map(([slotOffset, who, sol], order) => {
    const wallet = who === 'you' ? SAMPLE_WALLET : fakeAddress(who);
    const r = curveBuyTx({ curve, wallet, sol, slot: launch + slotOffset, blockTime: 1_760_000_000 + Math.floor(slotOffset * 0.4), signature: `sample-${order}`, mint: SAMPLE_MINT });
    curve = r.curve;
    return parseTrades(r.tx, SAMPLE_MINT).map((t) => ({ ...t, order }));
  }).flat();
  return analyzeMirror({ trades, userTrades: trades, wallet: SAMPLE_WALLET, signaturesBefore: BUYS.length - 1 });
}
