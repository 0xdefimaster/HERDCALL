// Read-only Solana JSON-RPC calls for Mirror Mode. Runs in the visitor's browser; no wallet signature, no transactions.
import { analyzeMirror, parseTrades } from './mirror.js';

export const DEFAULT_RPC = 'https://api.mainnet-beta.solana.com';
export const LIMITS = Object.freeze({ signaturePages: 20, earlyTransactions: 120, userTransactions: 15, concurrency: 3, retries: 8 });
// Public endpoints rate-limit per IP; space requests out a little there.
const PUBLIC_INTERVAL_MS = 120;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function rpcClient(url = DEFAULT_RPC, fetchFn = globalThis.fetch, { minIntervalMs = url === DEFAULT_RPC ? PUBLIC_INTERVAL_MS : 0, sleepFn = sleep } = {}) {
  let id = 0;
  let nextAt = 0;
  async function throttle() {
    if (minIntervalMs <= 0) return;
    const now = Date.now();
    const wait = Math.max(0, nextAt - now);
    nextAt = Math.max(now, nextAt) + minIntervalMs;
    if (wait > 0) await sleepFn(wait);
  }
  return async function call(method, params, attempt = 0) {
    await throttle();
    id += 1;
    const res = await fetchFn(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
    });
    if (res.status === 429 && attempt < LIMITS.retries) {
      const retryAfter = Number(res.headers?.get?.('retry-after'));
      await sleepFn(retryAfter > 0 ? retryAfter * 1000 : Math.min(500 * 2 ** attempt, 10_000));
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

// Newest transaction version we ask for. If the RPC reports a newer one, retry with the version it names.
let txVersion = 1;
async function getTx(call, signature) {
  try {
    return await call('getTransaction', [signature, { encoding: 'jsonParsed', maxSupportedTransactionVersion: txVersion, commitment: 'confirmed' }]);
  } catch (e) {
    const needed = /"maxSupportedTransactionVersion":\s*(\d+)/.exec(e.message);
    if (!needed || Number(needed[1]) <= txVersion) throw e;
    txVersion = Number(needed[1]);
    return getTx(call, signature);
  }
}

// Full Mirror run. `onProgress({ step, done, total })` for the UI.
export async function runMirror({ mint, wallet, buySignature, rpcUrl, fetchFn, sleepFn, onProgress = () => {} }) {
  const call = rpcClient(rpcUrl || DEFAULT_RPC, fetchFn, sleepFn ? { sleepFn } : undefined);
  const decode = (sigs, onDone) => mapLimit(sigs, LIMITS.concurrency, (s) => getTx(call, s.signature), onDone);

  onProgress({ step: 'signatures' });
  const { signatures, complete } = await allSignatures(call, mint);
  if (!complete) return { error: 'too-active', fetched: signatures.length };
  if (signatures.length === 0) return { error: 'no-history' };
  const order = new Map(signatures.map((s, i) => [s.signature, i]));
  const withOrder = (t) => ({ ...t, order: order.get(t.signature) ?? null });
  const tradesOf = (txs) => txs.flatMap((tx) => (tx ? parseTrades(tx, mint) : [])).map(withOrder);

  // 1. The user's first buy: from a pasted transaction, or the user's token account history (a few requests).
  onProgress({ step: 'user' });
  let userTrades = [];
  if (buySignature) userTrades = userTrades.concat(tradesOf([await getTx(call, buySignature)]));
  if (!userTrades.some((t) => t.wallet === wallet && t.side === 'buy')) {
    const accounts = await call('getTokenAccountsByOwner', [wallet, { mint }, { encoding: 'jsonParsed' }]);
    for (const acc of accounts?.value ?? []) {
      const sigs = await call('getSignaturesForAddress', [acc.pubkey, { limit: 1000 }]);
      const oldest = sigs.filter((s) => !s.err).reverse().slice(0, LIMITS.userTransactions);
      userTrades = userTrades.concat(tradesOf(await decode(oldest)));
    }
  }
  const knownBuy = userTrades.filter((t) => t.wallet === wallet && t.side === 'buy').sort((a, b) => a.slot - b.slot)[0];

  // 2. The token's first transactions, only up to the user's buy when we already know it.
  const window = (knownBuy ? signatures.filter((s) => s.slot <= knownBuy.slot) : signatures).slice(0, LIMITS.earlyTransactions);
  const trades = tradesOf(await decode(window, (done, total) => onProgress({ step: 'early', done, total })));
  userTrades = userTrades.concat(trades.filter((t) => t.wallet === wallet));

  const userBuy = userTrades.filter((t) => t.wallet === wallet && t.side === 'buy').sort((a, b) => a.slot - b.slot)[0];
  const signaturesBefore = userBuy ? signatures.filter((s) => s.slot < userBuy.slot).length : null;
  return analyzeMirror({ trades, userTrades, wallet, signaturesBefore });
}
