// Builds `getTransaction` (jsonParsed) shaped objects. Used by the tests and by the sample run on the Mirror page.
export function makeTx({ slot, blockTime = null, signature, mint, decimals = 6, fee = 5000, parties }) {
  const accountKeys = parties.map((p) => ({ pubkey: p.pubkey, signer: !!p.signer, writable: true, source: 'transaction' }));
  const preBalances = parties.map((p) => p.lamports ?? 5_000_000_000);
  const postBalances = parties.map((p, i) => preBalances[i] + (p.lamportDelta ?? 0) - (i === 0 ? fee : 0));
  const preTokenBalances = [];
  const postTokenBalances = [];
  parties.forEach((p, i) => {
    if (p.tokenPre == null && p.tokenPost == null) return;
    const entry = (amount) => ({ accountIndex: 100 + i, mint: p.mint ?? mint, owner: p.pubkey, uiTokenAmount: { amount: String(amount), decimals } });
    if (p.tokenPre != null) preTokenBalances.push(entry(p.tokenPre));
    if (p.tokenPost != null) postTokenBalances.push(entry(p.tokenPost));
  });
  return {
    slot,
    blockTime,
    meta: { err: null, fee, preBalances, postBalances, preTokenBalances, postTokenBalances },
    transaction: { signatures: [signature], message: { accountKeys } },
  };
}

// A buy on a constant-product curve. Returns the transaction and the new curve state.
export function curveBuyTx({ curve, wallet, sol, slot, blockTime, signature, mint, curveAccount = 'Curve1111111111111111111111111111111111111' }) {
  const k = curve.v * curve.t;
  const tokens = curve.t - k / (curve.v + sol);
  const raw = Math.floor(tokens * 1e6);
  const lamports = Math.round(sol * 1e9);
  const tx = makeTx({
    slot,
    blockTime,
    signature,
    mint,
    parties: [
      { pubkey: wallet, signer: true, lamportDelta: -lamports, tokenPost: raw },
      { pubkey: curveAccount, lamportDelta: lamports, tokenPre: 800_000_000_000_000, tokenPost: 800_000_000_000_000 - raw },
    ],
  });
  return { tx, curve: { v: curve.v + sol, t: curve.t - tokens } };
}
