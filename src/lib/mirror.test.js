// Checks for Mirror Mode parsing, analysis and the RPC client. Run: npm run test:mirror
import test from 'node:test';
import assert from 'node:assert/strict';
import { analyzeMirror, parseTrades, shareText } from './mirror.js';
import { curveBuyTx, makeTx } from './mirrorFixture.js';
import { projectShare, projectStampede } from './stampede.js';
import { runMirror } from './mirrorRpc.js';

const MINT = 'Mint111111111111111111111111111111111111111';
const CURVE = 'Curve1111111111111111111111111111111111111';

test('parseTrades: a signer buy, net of the network fee; the curve account is ignored', () => {
  const tx = makeTx({
    slot: 10,
    signature: 's1',
    mint: MINT,
    fee: 5000,
    parties: [
      { pubkey: 'alice', signer: true, lamportDelta: -2_000_000_000, tokenPost: 5_000_000 },
      { pubkey: CURVE, lamportDelta: 2_000_000_000, tokenPre: 9_000_000, tokenPost: 4_000_000 },
    ],
  });
  assert.deepEqual(parseTrades(tx, MINT), [{ wallet: 'alice', side: 'buy', tokens: 5, sol: 2, slot: 10, blockTime: null, signature: 's1' }]);
});

test('parseTrades: sells, other mints, failed transactions and legacy account keys', () => {
  const sell = makeTx({
    slot: 11,
    signature: 's2',
    mint: MINT,
    parties: [
      { pubkey: 'bob', signer: true, lamportDelta: 1_500_000_000, tokenPre: 3_000_000, tokenPost: 0 },
      { pubkey: CURVE, lamportDelta: -1_500_000_000, tokenPre: 1, tokenPost: 3_000_001 },
    ],
  });
  assert.deepEqual(parseTrades(sell, MINT).map((t) => [t.wallet, t.side, t.tokens, t.sol]), [['bob', 'sell', 3, 1.5]]);
  assert.deepEqual(parseTrades(sell, 'OtherMint'), []);
  assert.deepEqual(parseTrades({ ...sell, meta: { ...sell.meta, err: { InstructionError: [0, 'Custom'] } } }, MINT), []);
  const legacy = structuredClone(sell);
  legacy.transaction.message.accountKeys = legacy.transaction.message.accountKeys.map((k) => k.pubkey);
  legacy.transaction.message.header = { numRequiredSignatures: 1 };
  assert.equal(parseTrades(legacy, MINT).length, 1);
});

function launch(buys) {
  let curve = { v: 30, t: 1_073_000_000 };
  return buys.flatMap(([slot, wallet, sol], order) => {
    const r = curveBuyTx({ curve, wallet, sol, slot, signature: `t${order}`, mint: MINT });
    curve = r.curve;
    return parseTrades(r.tx, MINT).map((t) => ({ ...t, order }));
  });
}

test('analyzeMirror: wallets before you, first block, price multiple and the one-price counterfactual', () => {
  const trades = launch([
    [100, 'dev', 1],
    [100, 's1', 2],
    [100, 's2', 2],
    [101, 's3', 1],
    [101, 's1', 1], // same wallet twice counts once
    [105, 'me', 1],
    [106, 'late', 3],
  ]);
  const r = analyzeMirror({ trades, userTrades: trades, wallet: 'me' });
  assert.equal(r.walletsBefore, 4);
  assert.equal(r.firstBlockWallets, 3);
  assert.equal(r.slotsAfterLaunch, 5);
  assert.ok(Math.abs(r.beforeSol - 7) < 1e-9);
  assert.ok(r.priceMultiple > 1, 'later buyers pay more on a curve');
  assert.equal(r.partial, false);
  assert.ok(Math.abs(r.herd.mcapSol - projectStampede(8).mcapSol) < 1e-9);
  assert.ok(Math.abs(r.herd.tokens - projectShare(1, 8)) < 1e-9);
  assert.ok(r.herd.tokenGain > 0, 'one price gives the late buyer more tokens than the curve did');
});

test('analyzeMirror: order inside a slot, first buyer, missing buy, partial window', () => {
  const trades = launch([
    [100, 'a', 1],
    [100, 'me', 1],
    [100, 'b', 1],
  ]);
  assert.equal(analyzeMirror({ trades, userTrades: trades, wallet: 'me' }).walletsBefore, 1);
  const solo = launch([[100, 'me', 1], [101, 'x', 1]]);
  const first = analyzeMirror({ trades: solo, userTrades: solo, wallet: 'me' });
  assert.equal(first.walletsBefore, 0);
  assert.equal(first.priceMultiple, null);
  assert.match(shareText(first, 'ABC'), /Nobody got in before me on \$ABC/);
  assert.equal(analyzeMirror({ trades, userTrades: [], wallet: 'nobody' }).error, 'no-user-buy');
  const late = launch([[500, 'me', 1]]).map((t) => ({ ...t, order: 99 }));
  assert.equal(analyzeMirror({ trades, userTrades: late, wallet: 'me' }).partial, true);
});

test('shareText: names the count and the multiple', () => {
  const trades = launch([[100, 'a', 2], [100, 'b', 2], [103, 'me', 1]]);
  const text = shareText(analyzeMirror({ trades, userTrades: trades, wallet: 'me' }), 'BELL');
  assert.match(text, /^2 wallets got in before me on \$BELL\. I paid \d+\.\dx their average price\./);
});

// Fake RPC: signatures newest first, paged by `before`; one 429 on the first call to test the retry.
function fakeRpc({ txs, extraSignatures = 0, extraSlot = 10_000, userAccounts = [] }) {
  const sigs = [
    ...txs.map((tx) => ({ signature: tx.transaction.signatures[0], slot: tx.slot, err: null })),
    ...Array.from({ length: extraSignatures }, (_, i) => ({ signature: `extra${i}`, slot: extraSlot + i, err: null })),
  ].sort((a, b) => b.slot - a.slot);
  const bySig = new Map(txs.map((tx) => [tx.transaction.signatures[0], tx]));
  let first = true;
  const calls = [];
  const fetchFn = async (_url, init) => {
    const { method, params, id } = JSON.parse(init.body);
    calls.push(method);
    if (first) {
      first = false;
      return { status: 429, ok: false, json: async () => ({}) };
    }
    let result;
    if (method === 'getSignaturesForAddress') {
      const [address, opts] = params;
      const list = address === MINT ? sigs : userAccounts.find((a) => a.pubkey === address)?.sigs ?? [];
      const start = opts.before ? list.findIndex((s) => s.signature === opts.before) + 1 : 0;
      result = list.slice(start, start + opts.limit);
    } else if (method === 'getTransaction') result = bySig.get(params[0]) ?? null;
    else if (method === 'getTokenAccountsByOwner') result = { value: userAccounts.map((a) => ({ pubkey: a.pubkey })) };
    return { status: 200, ok: true, json: async () => ({ jsonrpc: '2.0', id, result }) };
  };
  return { fetchFn, calls };
}

function launchTxs(buys) {
  let curve = { v: 30, t: 1_073_000_000 };
  return buys.map(([slot, wallet, sol], i) => {
    const r = curveBuyTx({ curve, wallet, sol, slot, signature: `tx${i}`, mint: MINT });
    curve = r.curve;
    return r.tx;
  });
}

test('runMirror: end to end against a fake RPC, with a rate-limit retry', async () => {
  const txs = launchTxs([[100, 'dev', 1], [100, 's1', 2], [101, 's2', 1], [104, 'me', 1]]);
  const { fetchFn, calls } = fakeRpc({ txs });
  const steps = [];
  const r = await runMirror({ mint: MINT, wallet: 'me', fetchFn, onProgress: (p) => steps.push(p.step) });
  assert.equal(r.walletsBefore, 3);
  assert.equal(r.signaturesBefore, 3);
  assert.ok(calls.filter((c) => c === 'getSignaturesForAddress').length >= 2, 'retried after 429');
  assert.deepEqual([...new Set(steps)], ['signatures', 'early', 'user']);
});

test('runMirror: finds a late buy through the token account, and refuses tokens too active for the browser', async () => {
  const early = launchTxs([[100, 'dev', 1], [100, 's1', 2]]);
  const mine = launchTxs([[900, 'me', 1]])[0];
  mine.transaction.signatures[0] = 'mine';
  const { fetchFn } = fakeRpc({
    txs: [...early, mine],
    extraSignatures: 300,
    extraSlot: 200, // 300 transactions between launch and the user's buy: outside the decoded window
    userAccounts: [{ pubkey: 'myAta', sigs: [{ signature: 'mine', slot: 900, err: null }] }],
  });
  const r = await runMirror({ mint: MINT, wallet: 'me', fetchFn });
  assert.equal(r.userSlot, 900);
  assert.equal(r.walletsBefore, 2);
  assert.equal(r.signaturesBefore, 302);
  assert.equal(r.partial, true, 'counts are a lower bound');

  const busy = fakeRpc({ txs: early, extraSignatures: 25_000 });
  const tooActive = await runMirror({ mint: MINT, wallet: 'me', fetchFn: busy.fetchFn });
  assert.equal(tooActive.error, 'too-active');
});
