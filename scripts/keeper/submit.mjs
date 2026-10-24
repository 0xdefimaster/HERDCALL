// Sends keeper transactions: through Jito as a single-transaction bundle with a tip (not executed unless it lands
// whole, and not visible to the public before it lands), or straight to the RPC.
import anchor from '@anchor-lang/core';

const { web3 } = anchor;

async function jsonRpc(url, method, params) {
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const body = await res.json();
  if (body.error) throw new Error(`${method}: ${JSON.stringify(body.error)}`);
  return body.result;
}

/**
 * `mode`: 'rpc' or 'jito'. For Jito, `jitoUrl` is the block engine (e.g. https://mainnet.block-engine.jito.wtf) and
 * `tipLamports` the tip, paid only if the bundle lands.
 */
export function makeSubmitter({ connection, payer, mode = 'rpc', jitoUrl, tipLamports = 10_000, priorityMicroLamports = 0, computeUnits = 600_000 }) {
  let tipAccounts = null;
  const budget = () => [
    web3.ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnits }),
    ...(priorityMicroLamports > 0 ? [web3.ComputeBudgetProgram.setComputeUnitPrice({ microLamports: priorityMicroLamports })] : []),
  ];

  async function build(ixs, withTip) {
    const tx = new web3.Transaction().add(...budget(), ...ixs);
    if (withTip) {
      tipAccounts ??= await jsonRpc(`${jitoUrl}/api/v1/bundles`, 'getTipAccounts', []);
      const tip = new web3.PublicKey(tipAccounts[Math.floor(Math.random() * tipAccounts.length)]);
      tx.add(web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: tip, lamports: tipLamports }));
    }
    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash('confirmed');
    tx.recentBlockhash = blockhash;
    tx.feePayer = payer.publicKey;
    tx.sign(payer);
    return { tx, lastValidBlockHeight };
  }

  return {
    /** Simulates first; returns `{ ok: false, logs }` without sending if the program would fail. */
    async send(ixs) {
      const sim = await build(ixs, false);
      const r = await connection.simulateTransaction(sim.tx);
      if (r.value.err) return { ok: false, err: r.value.err, logs: r.value.logs ?? [] };
      const { tx, lastValidBlockHeight } = await build(ixs, mode === 'jito');
      const raw = tx.serialize();
      let signature;
      if (mode === 'jito') {
        await jsonRpc(`${jitoUrl}/api/v1/bundles`, 'sendBundle', [[raw.toString('base64')], { encoding: 'base64' }]);
        signature = anchor.utils.bytes.bs58.encode(tx.signature);
      } else {
        signature = await connection.sendRawTransaction(raw, { skipPreflight: true });
      }
      try {
        const c = await connection.confirmTransaction({ signature, blockhash: tx.recentBlockhash, lastValidBlockHeight }, 'confirmed');
        if (c.value.err) return { ok: false, err: c.value.err, signature, logs: [] };
      } catch (e) {
        // A bundle that did not land, or a transaction that expired: nothing was executed or paid.
        return { ok: false, err: String(e?.message ?? e), signature, logs: [] };
      }
      return { ok: true, signature };
    },
  };
}
