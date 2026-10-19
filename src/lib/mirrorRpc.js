// Read-only Solana JSON-RPC calls for Mirror Mode. Runs in the visitor's browser; no wallet signature, no transactions.
import { analyzeMirror, parseTrades } from './mirror.js';

export const DEFAULT_RPC = 'https://api.mainnet-beta.solana.com';
export const LIMITS = Object.freeze({ signaturePages: 20, earlyTransactions: 120, userTransactions: 15, concurrency: 4 });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function rpcClient(url = DEFAULT_RPC, fetchFn = globalThis.fetch) {
  let id = 0;
  return async function call(method, params, attempt = 0) {
    id += 1;
    const res = await fetchFn(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
    });
    if (res.status === 429 && attempt < 5) {
      await sleep(500 * 2 ** attempt);
      return call(method, params, attempt + 1);
    }
    if (!res.ok) throw new Error(`RPC ${method} failed: HTTP ${res.status}`);
    const body = await res.json();
    if (body.error) throw new Error(`RPC ${method} failed: ${body.error.message}`);
    return body.result;
  };
}

// All successful signatures of an address, oldest first. Stops after `maxPages` pages of 1000.
export async function allSignatures(call, address, maxPages = LIMITS.signaturePages) {
  const out = [];
  let before;
  for (let page = 0; page < maxPages; page += 1) {
    const batch = await call('getSignaturesForAddress', [address, { limit: 1000, ...(before ? { before } : {}) }]);
    out.push(...batch);
    if (batch.length < 1000) return { signatures: out.filter((s) => !s.err).reverse(), complete: true };
    before = batch[batch.length - 1].signature;
  }
  return { signatures: out.filter((s) => !s.err).reverse(), complete: false };
}

async function mapLimit(items, limit, fn, onDone) {
  const results = new Array(items.length);
  let next = 0;
  let done = 0;
  async function worker() {
    while (next < items.length) {
      const i = next;
      next += 1;
      results[i] = await fn(items[i], i);
      done += 1;
      onDone?.(done, items.length);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

const getTx = (call, signature) =>
  call('getTransaction', [signature, { encoding: 'jsonParsed', maxSupportedTransactionVersion: 0, commitment: 'confirmed' }]);

// Full Mirror run. `onProgress({ step, done, total })` for the UI.
export async function runMirror({ mint, wallet, buySignature, rpcUrl, fetchFn, onProgress = () => {} }) {
  const call = rpcClient(rpcUrl || DEFAULT_RPC, fetchFn);

  onProgress({ step: 'signatures' });
  const { signatures, complete } = await allSignatures(call, mint);
  if (!complete) return { error: 'too-active', fetched: signatures.length };
  if (signatures.length === 0) return { error: 'no-history' };
  const order = new Map(signatures.map((s, i) => [s.signature, i]));
  const withOrder = (t) => ({ ...t, order: order.get(t.signature) ?? null });

  const early = signatures.slice(0, LIMITS.earlyTransactions);
  const earlyTxs = await mapLimit(early, LIMITS.concurrency, (s) => getTx(call, s.signature), (done, total) =>
    onProgress({ step: 'early', done, total }),
  );
  const trades = earlyTxs.flatMap((tx) => (tx ? parseTrades(tx, mint) : [])).map(withOrder);

  // The user's own buys: from the decoded window, an explicit transaction, or the user's token account history.
  onProgress({ step: 'user' });
  let userTrades = trades.filter((t) => t.wallet === wallet);
  if (buySignature) {
    const tx = await getTx(call, buySignature);
    if (tx) userTrades = userTrades.concat(parseTrades(tx, mint).map(withOrder));
  }
  if (!userTrades.some((t) => t.side === 'buy')) {
    const accounts = await call('getTokenAccountsByOwner', [wallet, { mint }, { encoding: 'jsonParsed' }]);
    for (const acc of accounts?.value ?? []) {
      const sigs = await call('getSignaturesForAddress', [acc.pubkey, { limit: 1000 }]);
      const oldest = sigs.filter((s) => !s.err).reverse().slice(0, LIMITS.userTransactions);
      const txs = await mapLimit(oldest, LIMITS.concurrency, (s) => getTx(call, s.signature));
      userTrades = userTrades.concat(txs.flatMap((tx) => (tx ? parseTrades(tx, mint) : [])).map(withOrder));
    }
  }

  const userBuy = userTrades.filter((t) => t.wallet === wallet && t.side === 'buy').sort((a, b) => a.slot - b.slot)[0];
  const signaturesBefore = userBuy ? signatures.filter((s) => s.slot < userBuy.slot).length : null;
  return analyzeMirror({ trades, userTrades, wallet, signaturesBefore });
}
