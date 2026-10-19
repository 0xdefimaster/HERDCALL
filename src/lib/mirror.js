// Mirror Mode (document/HERDCALL.md §10): who bought a token before you, and what one-price entry would have looked like.
// Pure functions over Solana `getTransaction` (jsonParsed) results. No platform-specific instruction decoding:
// trades are read from token and SOL balance changes of the transaction's signers.
import { projectShare, projectStampede } from './stampede.js';

const LAMPORTS = 1e9;

function keyOf(k) {
  return typeof k === 'string' ? k : k.pubkey;
}

// Buys and sells of `mint` in one transaction, one entry per signer whose balance of the mint changed.
// SOL amounts include platform fees and account rent, so they are approximate. Non-signers (pools, curves) are ignored.
export function parseTrades(tx, mint) {
  if (!tx?.meta || tx.meta.err) return [];
  const keys = tx.transaction.message.accountKeys;
  const signer = (i) => (typeof keys[i] === 'string' ? i < (tx.transaction.message.header?.numRequiredSignatures ?? 1) : keys[i].signer);
  const tokenDelta = new Map();
  let decimals = 0;
  for (const [list, sign] of [[tx.meta.preTokenBalances ?? [], -1], [tx.meta.postTokenBalances ?? [], 1]]) {
    for (const b of list) {
      if (b.mint !== mint || !b.owner) continue;
      decimals = b.uiTokenAmount.decimals;
      tokenDelta.set(b.owner, (tokenDelta.get(b.owner) ?? 0n) + BigInt(sign) * BigInt(b.uiTokenAmount.amount));
    }
  }
  const trades = [];
  for (const [owner, raw] of tokenDelta) {
    if (raw === 0n) continue;
    const i = keys.findIndex((k) => keyOf(k) === owner);
    if (i < 0 || !signer(i)) continue;
    let lamports = tx.meta.postBalances[i] - tx.meta.preBalances[i];
    if (i === 0) lamports += tx.meta.fee; // the network fee is not part of the trade
    const tokens = Number(raw) / 10 ** decimals;
    const sol = lamports / LAMPORTS;
    if (tokens > 0 && sol < 0) trades.push({ wallet: owner, side: 'buy', tokens, sol: -sol });
    else if (tokens < 0 && sol > 0) trades.push({ wallet: owner, side: 'sell', tokens: -tokens, sol });
  }
  return trades.map((t) => ({ ...t, slot: tx.slot, blockTime: tx.blockTime ?? null, signature: tx.transaction.signatures?.[0] ?? null }));
}

// Order inside a slot is unknown from signatures alone; `order` keeps the position from the signature list.
export function sortTrades(trades) {
  return [...trades].sort((a, b) => a.slot - b.slot || (a.order ?? 0) - (b.order ?? 0));
}

// Main analysis.
//   trades       buys/sells of the token's earliest decoded transactions (any order)
//   userTrades   the user's own trades of the token (any order); may overlap `trades`
//   wallet       the user's address
//   signaturesBefore  optional: how many successful transactions happened before the user's first buy (from the
//                     signature list, which covers more history than the decoded window)
export function analyzeMirror({ trades, userTrades, wallet, signaturesBefore = null }) {
  const ordered = sortTrades(trades);
  const mine = sortTrades(userTrades.filter((t) => t.wallet === wallet && t.side === 'buy'));
  if (mine.length === 0) return { error: 'no-user-buy' };
  if (ordered.length === 0) return { error: 'no-trades' };
  const first = mine[0];
  const launchSlot = Math.min(ordered[0].slot, first.slot);
  const isBefore = (t) => t.slot < first.slot || (t.slot === first.slot && t.order != null && first.order != null && t.order < first.order);
  const beforeBuys = ordered.filter((t) => t.side === 'buy' && t.wallet !== wallet && isBefore(t));
  const wallets = new Map();
  for (const t of beforeBuys) {
    const w = wallets.get(t.wallet) ?? { wallet: t.wallet, sol: 0, tokens: 0, slot: t.slot };
    w.sol += t.sol;
    w.tokens += t.tokens;
    wallets.set(t.wallet, w);
  }
  const before = [...wallets.values()].sort((a, b) => a.slot - b.slot || b.sol - a.sol);
  const beforeSol = before.reduce((a, w) => a + w.sol, 0);
  const beforeTokens = before.reduce((a, w) => a + w.tokens, 0);
  const userPrice = first.sol / first.tokens;
  const theirPrice = beforeTokens > 0 ? beforeSol / beforeTokens : null;
  const decodedUntil = ordered[ordered.length - 1].slot;

  // One-price counterfactual: everyone who bought up to and including the user's first buy sits in one Lobby.
  const pool = beforeSol + first.sol;
  const stampede = projectStampede(pool);
  const herdTokens = projectShare(first.sol, pool);

  return {
    wallet,
    launchSlot,
    userSlot: first.slot,
    slotsAfterLaunch: first.slot - launchSlot,
    secondsAfterLaunch: first.blockTime != null && ordered[0].blockTime != null ? first.blockTime - ordered[0].blockTime : null,
    userBuy: first,
    userPrice,
    walletsBefore: before.length,
    firstBlockWallets: before.filter((w) => w.slot === launchSlot).length,
    before,
    beforeSol,
    theirPrice,
    priceMultiple: theirPrice ? userPrice / theirPrice : null,
    signaturesBefore,
    partial: first.slot > decodedUntil, // the user bought after the decoded window: counts are a lower bound
    herd: stampede && {
      pool,
      mcapSol: stampede.mcapSol,
      tokens: herdTokens,
      tokenGain: first.tokens > 0 ? herdTokens / first.tokens - 1 : null,
    },
  };
}

export function shareText(result, ticker) {
  const name = ticker ? `$${ticker}` : 'this token';
  if (result.walletsBefore === 0) return `Nobody got in before me on ${name}. On HerdCall nobody ever does: everyone enters at one price. herdcall.fun/mirror`;
  const multiple = result.priceMultiple ? ` I paid ${result.priceMultiple.toFixed(1)}x their average price.` : '';
  return `${result.walletsBefore} wallets got in before me on ${name}.${multiple} On HerdCall everyone enters at one price. herdcall.fun/mirror`;
}
