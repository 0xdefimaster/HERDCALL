// End-to-end check of the keeper on a running cluster (local validator or devnet) with a migrated launch from
// scripts/launch-flow.mjs. Uses a fresh keeper key (never the admin's), short windows so it runs in minutes, a mock
// Jito block engine that forwards bundles to the cluster, and a mock alert webhook.
//
//   node scripts/keeper/scenario.mjs --url http://127.0.0.1:8899 --keypair <admin> --dex-config <config>
import anchor from '@anchor-lang/core';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  WSOL, ata, createAtaIx, dexSwapIx, globalKeys, launchKeys, readPool, reinvestCap, wrapSolIxs,
} from '../herdcall-client.mjs';

const { web3 } = anchor;
const SOL = 1_000_000_000;
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};
const url = arg('url', 'http://127.0.0.1:8899');
const adminPath = arg('keypair', `${homedir()}/.config/solana/id.json`).replace(/^~/, homedir());
const dexConfig = new web3.PublicKey(arg('dex-config'));
const volume = Number(arg('volume-sol', '40'));
// Volume is traded in round trips of this size (a small pool, as on devnet, needs small trips).
const chunk = Number(arg('chunk-sol', String(volume / 2)));

const admin = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(adminPath, 'utf8'))));
const connection = new web3.Connection(url, 'confirmed');
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(admin), { commitment: 'confirmed' });
const idl = JSON.parse(readFileSync(new URL('../../programs/idl/herdcall.json', import.meta.url), 'utf8'));
const program = new anchor.Program(idl, provider);
const g = globalKeys();
const dir = mkdtempSync(join(tmpdir(), 'herdcall-keeper-'));

const step = (m) => console.log(`\n▸ ${m}`);
function check(cond, msg) {
  if (!cond) throw new Error(`check failed: ${msg}`);
  console.log(`  ✓ ${msg}`);
}
const send = (ixs, signers) => web3.sendAndConfirmTransaction(connection, new web3.Transaction().add(...ixs), signers);

// Mock services: a Jito block engine that forwards each bundle's transaction to the cluster, and an alert webhook.
const tipAccount = web3.Keypair.generate().publicKey;
const seen = { bundles: 0, alerts: [] };
const server = createServer(async (req, res) => {
  let body = '';
  for await (const c of req) body += c;
  const msg = JSON.parse(body);
  let result = null;
  if (req.url === '/alert') seen.alerts.push(msg);
  else if (msg.method === 'getTipAccounts') result = [tipAccount.toBase58()];
  else if (msg.method === 'sendBundle') {
    seen.bundles += 1;
    for (const b64 of msg.params[0]) await connection.sendRawTransaction(Buffer.from(b64, 'base64'), { skipPreflight: true });
    result = `bundle-${seen.bundles}`;
  }
  res.setHeader('content-type', 'application/json');
  res.end(JSON.stringify({ jsonrpc: '2.0', id: msg.id ?? 1, result }));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const mock = `http://127.0.0.1:${server.address().port}`;

function runKeeper(keeperPath, secs, extra = []) {
  return new Promise((resolve, reject) => {
    const p = spawn(process.execPath, [
      new URL('./keeper.mjs', import.meta.url).pathname, '--url', url, '--keypair', keeperPath, '--state', join(dir, 'state.json'),
      '--sample-secs', '3', '--window-secs', '45', '--min-samples', '8', '--claim-every-secs', '20', '--reinvest-every-secs', '10',
      '--run-for-secs', String(secs), ...extra,
    ], { env: process.env });
    let out = '';
    p.stdout.on('data', (d) => { out += d; });
    p.stderr.on('data', (d) => { out += d; });
    p.on('exit', (code) => {
      const events = out.trim().split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l));
      code === 0 ? resolve(events) : reject(new Error(`keeper exited ${code}:\n${out}`));
    });
  });
}

const latest = async () => {
  const all = (await program.account.launch.all()).filter((a) => 'migrated' in a.account.phase);
  if (all.length === 0) throw new Error('no migrated launch: run scripts/launch-flow.mjs first');
  return all.sort((a, b) => (a.account.closeAt.gt(b.account.closeAt) ? -1 : 1))[0];
};

try {
  const { publicKey: launch, account: l0 } = await latest();
  const k = launchKeys(l0.mint, l0.ticker, l0.name);
  const poolSqrt = async () => readPool((await connection.getAccountInfo(l0.dexPool)).data).sqrtPrice;
  console.log(`launch ${launch.toBase58()} (${l0.ticker}), pool ${l0.dexPool.toBase58()}`);

  step('a keeper key of its own');
  const keeper = web3.Keypair.generate();
  const keeperPath = join(dir, 'keeper.json');
  writeFileSync(keeperPath, JSON.stringify(Array.from(keeper.secretKey)));
  await send([web3.SystemProgram.transfer({ fromPubkey: admin.publicKey, toPubkey: keeper.publicKey, lamports: 0.2 * SOL })], [admin]);
  const wrong = await runKeeper(adminPath, 1).then(() => null, (e) => String(e));
  check(wrong && wrong.includes('refusing to run with the admin key'), 'the admin key is refused');
  const notYet = await runKeeper(keeperPath, 1).then(() => null, (e) => String(e));
  check(notYet && notYet.includes('is not the reinvest keeper'), 'a key the admin has not appointed is refused');
  // The mock tip account must exist (rent) like Jito's real ones.
  await send([web3.SystemProgram.transfer({ fromPubkey: admin.publicKey, toPubkey: tipAccount, lamports: 0.001 * SOL })], [admin]);
  await program.methods.setReinvestKeeper(keeper.publicKey).accountsStrict({ admin: admin.publicKey, config: g.config }).rpc();
  check((await program.account.config.fetch(g.config)).reinvestKeeper.equals(keeper.publicKey), `keeper ${keeper.publicKey.toBase58()} appointed`);

  step(`${volume} SOL of volume on the pool, so fees accrue`);
  const trader = web3.Keypair.generate();
  await send([web3.SystemProgram.transfer({ fromPubkey: admin.publicKey, toPubkey: trader.publicKey, lamports: Math.round((chunk + 0.5) * SOL) })], [admin]);
  await send([createAtaIx(trader.publicKey, trader.publicKey, WSOL), createAtaIx(trader.publicKey, trader.publicKey, k.mint), ...wrapSolIxs(trader.publicKey, Math.round(chunk * SOL))], [trader]);
  const buy = async (sol) => send([dexSwapIx(trader.publicKey, k, dexConfig, Math.round(sol * SOL), true)], [trader]);
  const sellAll = async () => {
    const bal = BigInt((await connection.getTokenAccountBalance(ata(trader.publicKey, k.mint))).value.amount);
    await send([dexSwapIx(trader.publicKey, k, dexConfig, bal, false)], [trader]);
  };
  const wsolBalance = async () => Number((await connection.getTokenAccountBalance(ata(trader.publicKey, WSOL))).value.amount) / SOL;
  for (let done = 0; done < volume / 2; done += chunk) {
    await buy(Math.min(chunk, await wsolBalance()));
    await sellAll();
  }
  check(true, `traded ${volume} SOL in round trips of ${chunk} SOL`);

  step('first run: claims at once, but no median yet, so no reinvest');
  let ev = await runKeeper(keeperPath, 15, ['--alert-pending-secs', '5', '--alert-webhook', `${mock}/alert`]);
  check(ev.some((e) => e.event === 'claimed'), 'claimed the fees');
  let l = await program.account.launch.fetch(launch);
  check(l.lpReinvestPending.toNumber() >= 10_000_000, `${l.lpReinvestPending.toNumber() / SOL} SOL waiting to be reinvested`);
  check(ev.some((e) => e.event === 'reinvest-waiting' && /no median/.test(e.reason)), 'waits for 45 s of price history');
  check(!ev.some((e) => e.event === 'reinvested'), 'nothing reinvested');
  check(ev.some((e) => e.event === 'ALERT') && seen.alerts.length > 0, 'the waiting share raised an alert, sent to the webhook');

  step('price pushed just before: the keeper does not follow it');
  // Builds price history without reinvesting (no median allowed during this run).
  await runKeeper(keeperPath, 40, ['--min-samples', '100000']);
  const before = await poolSqrt();
  await buy(Math.min(chunk / 2, await wsolBalance()));
  const pushed = await poolSqrt();
  console.log(`  price pushed by ${((Number(pushed) / Number(before)) ** 2 * 100 - 100).toFixed(1)}%`);
  const waitingAtPush = (await program.account.launch.fetch(launch)).lpReinvestPending.toNumber();
  check(waitingAtPush >= 10_000_000, `${waitingAtPush / SOL} SOL waiting when the price is pushed`);
  ev = await runKeeper(keeperPath, 8);
  check(ev.some((e) => e.event === 'reinvest-waiting' && /outside the band/.test(e.reason)), 'outside the band around its median: waits');
  check(!ev.some((e) => e.event === 'reinvested'), 'nothing reinvested at the pushed price');

  step('price back: reinvests through Jito with a tip');
  await sellAll();
  const reinvestedBefore = (await program.account.launch.fetch(launch)).lpReinvested.toNumber();
  const cap = Number(reinvestCap(readPool((await connection.getAccountInfo(l0.dexPool)).data)));
  const expected = Math.min(waitingAtPush, cap);
  ev = await runKeeper(keeperPath, 12, ['--submit', 'jito', '--jito-url', mock, '--tip-lamports', '5000']);
  l = await program.account.launch.fetch(launch);
  if (!ev.some((e) => e.event === 'reinvested')) console.log(ev.filter((e) => e.launch === launch.toBase58() || e.event === 'error').slice(-6));
  check(ev.some((e) => e.event === 'reinvested'), 'reinvested');
  check(l.lpReinvested.toNumber() - reinvestedBefore >= expected * 0.98,
    `${(l.lpReinvested.toNumber() - reinvestedBefore) / SOL} SOL added to the locked position (waiting ${waitingAtPush / SOL}, cap per call ${cap / SOL})`);
  check(seen.bundles > 0 && (await connection.getBalance(tipAccount)) >= 0.001 * SOL + 5000, 'sent as a bundle; the tip reached the tip account');

  step('monitoring: health endpoint and a low balance');
  const port = 20_000 + Math.floor(Math.random() * 20_000);
  const alertsBefore = seen.alerts.length;
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const [, health] = await Promise.all([
    runKeeper(keeperPath, 10, ['--health-port', String(port), '--min-balance-sol', '1000', '--alert-webhook', `${mock}/alert`]),
    sleep(6000).then(() => fetch(`http://127.0.0.1:${port}/health`)).then(async (r) => ({ status: r.status, body: await r.json() })),
  ]);
  check(health.status === 200 && health.body.isKeeper && health.body.launches[launch.toBase58()]?.samples > 0,
    `GET /health: 200, ${health.body.launches[launch.toBase58()]?.samples} price samples for the launch`);
  const low = seen.alerts.slice(alertsBefore).filter((x) => x.kind === 'balance');
  check(low.length === 1, 'balance below the threshold: one alert (throttled, not one per loop)');

  step('monitoring: the admin appoints another keeper while it runs');
  const other = web3.Keypair.generate();
  const replacedAt = seen.alerts.length;
  const [events, after] = await Promise.all([
    runKeeper(keeperPath, 14, ['--refresh-secs', '2', '--health-port', String(port + 1), '--alert-webhook', `${mock}/alert`]),
    sleep(4000)
      .then(() => program.methods.setReinvestKeeper(other.publicKey).accountsStrict({ admin: admin.publicKey, config: g.config }).rpc())
      .then(() => sleep(6000))
      .then(() => fetch(`http://127.0.0.1:${port + 1}/health`))
      .then(async (r) => ({ status: r.status, body: await r.json() })),
  ]);
  check(events.some((e) => e.event === 'not-keeper'), 'the keeper notices it was replaced');
  check(seen.alerts.slice(replacedAt).some((x) => x.kind === 'not-keeper'), 'and alerts');
  check(after.status === 503 && after.body.reasons.includes('not the reinvest keeper'), 'GET /health: 503, not the reinvest keeper');
  const lateReinvest = events.filter((e) => e.event === 'reinvested' && Date.parse(e.at) > Date.parse(events.find((x) => x.event === 'not-keeper').at));
  check(lateReinvest.length === 0, 'no reinvest attempted after the replacement');
  console.log('\nKeeper scenario passed.');
} finally {
  server.close();
}
