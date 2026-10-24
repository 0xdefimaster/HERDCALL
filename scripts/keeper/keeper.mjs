// HerdCall keeper (CURVE_SPEC §5.7): samples every migrated launch's pool price, claims the locked positions' fees on
// a schedule and reinvests the "back to liquidity" share with a band around its own 30-minute median. Alerts when a
// share has waited more than a day.
//
//   node scripts/keeper/keeper.mjs --url <rpc> --keypair <keeper key> [--submit rpc|jito --jito-url <block engine>]
//     [--alert-webhook <url>] [--health-port <port>] [--min-balance-sol 0.05] [--state keeper-state.json]
//
// Monitoring (monitor.mjs): alerts for a low keeper balance, a key the admin has replaced, a failing loop and sends
// that keep failing, throttled to one per kind every 6 hours; GET /health on --health-port answers 200 or 503 with the
// keeper's state per launch.
//
// The keeper key must be its own key: the program's Config.reinvest_keeper, set by the admin with set_reinvest_keeper,
// and not the admin's key. The keeper can only reinvest within the program's limits; it cannot withdraw anything.
import anchor from '@anchor-lang/core';
import { existsSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { homedir } from 'node:os';
import { claimLpFeesAccounts, globalKeys, launchKeys, readPool, reinvestCap, reinvestLpAccounts } from '../herdcall-client.mjs';
import { DEFAULTS, addSample, decide } from './logic.mjs';
import { MONITOR_DEFAULTS, healthAlerts, healthStatus, recordSend, throttle } from './monitor.mjs';
import { makeSubmitter } from './submit.mjs';

const { web3, BN } = anchor;

const arg = (name, fallback) => {
  const i = process.argv.lastIndexOf(`--${name}`); // the last one wins
  return i > 0 ? process.argv[i + 1] : fallback;
};
const num = (name, fallback) => Number(arg(name, fallback));
const cfg = {
  ...DEFAULTS,
  sampleSecs: num('sample-secs', DEFAULTS.sampleSecs),
  windowSecs: num('window-secs', DEFAULTS.windowSecs),
  minSamples: num('min-samples', DEFAULTS.minSamples),
  claimEverySecs: num('claim-every-secs', DEFAULTS.claimEverySecs),
  reinvestEverySecs: num('reinvest-every-secs', DEFAULTS.reinvestEverySecs),
  alertPendingSecs: num('alert-pending-secs', DEFAULTS.alertPendingSecs),
  // Launches and the keeper appointment are re-read this often.
  refreshSecs: num('refresh-secs', 300),
};
const monitorCfg = {
  ...MONITOR_DEFAULTS,
  minBalanceLamports: BigInt(Math.round(num('min-balance-sol', 0.05) * 1e9)),
  alertRepeatSecs: num('alert-repeat-secs', MONITOR_DEFAULTS.alertRepeatSecs),
};
const healthPort = num('health-port', 0);
const url = arg('url', 'http://127.0.0.1:8899');
const keypairPath = arg('keypair', `${homedir()}/.config/herdcall/keeper.json`).replace(/^~/, homedir());
const statePath = arg('state', 'keeper-state.json');
const webhook = arg('alert-webhook', process.env.KEEPER_ALERT_WEBHOOK);
const runForSecs = num('run-for-secs', 0); // 0: forever

const keeper = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(keypairPath, 'utf8'))));
const connection = new web3.Connection(url, 'confirmed');
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(keeper), { commitment: 'confirmed' });
const idl = JSON.parse(readFileSync(new URL('../../programs/idl/herdcall.json', import.meta.url), 'utf8'));
const program = new anchor.Program(idl, provider);
const submitter = makeSubmitter({
  connection,
  payer: keeper,
  mode: arg('submit', 'rpc'),
  jitoUrl: arg('jito-url', 'https://mainnet.block-engine.jito.wtf'),
  tipLamports: num('tip-lamports', 10_000),
  priorityMicroLamports: num('priority-micro-lamports', 0),
});

const log = (event, data = {}) => console.log(JSON.stringify({ at: new Date().toISOString(), event, ...data }, (_, v) => (typeof v === 'bigint' ? v.toString() : v)));

async function alert(message, data) {
  log('ALERT', { message, ...data });
  if (!webhook) return;
  try {
    await fetch(webhook, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ text: `HerdCall keeper: ${message}`, ...data }) });
  } catch (e) {
    log('alert-webhook-failed', { error: String(e) });
  }
}

// What the keeper remembers per launch survives restarts, so a restart does not wait 30 minutes for a new median.
function loadState() {
  if (!existsSync(statePath)) return {};
  const raw = JSON.parse(readFileSync(statePath, 'utf8'));
  for (const s of Object.values(raw)) s.samples = (s.samples ?? []).map((x) => ({ t: x.t, sqrt: BigInt(x.sqrt) }));
  return raw;
}
function saveState(state) {
  const tmp = `${statePath}.tmp`;
  writeFileSync(tmp, JSON.stringify(state, (_, v) => (typeof v === 'bigint' ? v.toString() : v)));
  renameSync(tmp, statePath);
}

const isFailure = (r, code) => !r.ok && r.logs.some((l) => l.includes(code));

async function main() {
  const g = globalKeys();
  const config = await program.account.config.fetch(g.config);
  if (keeper.publicKey.equals(config.admin)) {
    throw new Error('refusing to run with the admin key: create a keeper key and have the admin call set_reinvest_keeper');
  }
  if (!config.reinvestKeeper.equals(keeper.publicKey)) {
    throw new Error(`this key (${keeper.publicKey.toBase58()}) is not the reinvest keeper (${config.reinvestKeeper.toBase58()}); the admin must call set_reinvest_keeper`);
  }
  log('start', { keeper: keeper.publicKey.toBase58(), url, cfg, submit: arg('submit', 'rpc') });
  const state = loadState();
  const started = Date.now();
  let launches = [];
  let refreshedAt = 0;
  // Monitoring state, in memory: a restart re-sends standing alerts once.
  const health = { isKeeper: true, balance: null, loopErrors: 0, lastLoopAt: null, sendFailures: {}, sent: {}, lastAlerts: [] };
  const server = healthPort ? startHealthServer(health, state) : null;

  for (;;) {
    const now = Math.floor(Date.now() / 1000);
    try {
      if (now - refreshedAt >= cfg.refreshSecs || launches.length === 0) {
        launches = (await program.account.launch.all()).filter((a) => 'migrated' in a.account.phase);
        const appointed = (await program.account.config.fetch(g.config)).reinvestKeeper;
        if (health.isKeeper && !appointed.equals(keeper.publicKey)) log('not-keeper', { appointed: appointed.toBase58() });
        health.isKeeper = appointed.equals(keeper.publicKey);
        refreshedAt = now;
        log('launches', { migrated: launches.length });
      }
      health.balance = BigInt(await connection.getBalance(keeper.publicKey, 'confirmed'));
      const pools = await connection.getMultipleAccountsInfo(launches.map((a) => a.account.dexPool), 'confirmed');
      for (const [i, { publicKey, account: l }] of launches.entries()) {
        const id = publicKey.toBase58();
        const s = state[id] ?? { samples: [] };
        if (!pools[i]) {
          log('pool-missing', { launch: id });
          continue;
        }
        const poolState = readPool(pools[i].data);
        const current = poolState.sqrtPrice;
        s.samples = addSample(s.samples, now, current, cfg.windowSecs);
        const fresh = await program.account.launch.fetch(publicKey);
        const k = launchKeys(fresh.mint, fresh.ticker, fresh.name);
        const pool = { pool: fresh.dexPool };
        const d = decide({ now, pending: BigInt(fresh.lpReinvestPending.toString()), current, cap: reinvestCap(poolState), state: s, cfg });
        Object.assign(s, d.state);

        if (d.alert) await alert(d.alert, { launch: id, ticker: fresh.ticker });
        if (d.claim) {
          const ix = await program.methods.claimLpFees().accountsStrict(claimLpFeesAccounts(keeper.publicKey, k, pool)).instruction();
          const r = await submitter.send([ix]);
          if (r.ok) log('claimed', { launch: id, signature: r.signature });
          else if (isFailure(r, 'NoFees')) log('claim-skipped', { launch: id, reason: 'no fees' });
          else log('claim-failed', { launch: id, err: r.err, logs: r.logs.slice(-5) });
          health.sendFailures = recordSend(health.sendFailures, id, 'claim', isFailure(r, 'NoFees') ? { ok: true } : r);
          s.lastClaimAt = now;
        }
        if (d.reinvest && !health.isKeeper) {
          log('reinvest-waiting', { launch: id, reason: 'not the reinvest keeper' });
        } else if (d.reinvest) {
          const [lo, hi] = d.reinvest.band;
          const ix = await program.methods.reinvestLp(new BN(lo.toString()), new BN(hi.toString()))
            .accountsStrict(reinvestLpAccounts(keeper.publicKey, k, pool)).instruction();
          const r = await submitter.send([ix]);
          health.sendFailures = recordSend(health.sendFailures, id, 'reinvest', r);
          if (r.ok) {
            s.lastReinvestAt = now;
            log('reinvested', { launch: id, signature: r.signature });
          } else {
            log('reinvest-failed', { launch: id, err: r.err, logs: r.logs.slice(-5) });
          }
        } else if (d.skip && fresh.lpReinvestPending.gtn(0)) {
          log('reinvest-waiting', { launch: id, reason: d.skip });
        }
        state[id] = s;
      }
      saveState(state);
      health.loopErrors = 0;
      health.lastLoopAt = now;
    } catch (e) {
      health.loopErrors += 1;
      log('error', { error: String(e?.message ?? e), consecutive: health.loopErrors });
    }
    const alerts = healthAlerts({ ...health, cfg: monitorCfg });
    const t = throttle(health.sent, now, alerts, monitorCfg.alertRepeatSecs);
    health.sent = t.sent;
    health.lastAlerts = alerts.map((a) => a.message);
    for (const a of t.send) await alert(a.message, { kind: a.key, ...(a.launch ? { launch: a.launch } : {}) });
    if (runForSecs > 0 && (Date.now() - started) / 1000 >= runForSecs) break;
    await new Promise((r) => setTimeout(r, cfg.sampleSecs * 1000));
  }
  server?.close();
  log('stop');
}

function startHealthServer(health, state) {
  const server = createServer((req, res) => {
    const now = Math.floor(Date.now() / 1000);
    const status = healthStatus({ now, lastLoopAt: health.lastLoopAt, sampleSecs: cfg.sampleSecs, isKeeper: health.isKeeper, loopErrors: health.loopErrors });
    const body = {
      ...status,
      keeper: keeper.publicKey.toBase58(),
      isKeeper: health.isKeeper,
      balanceLamports: health.balance,
      lastLoopAt: health.lastLoopAt,
      loopErrors: health.loopErrors,
      alerts: health.lastAlerts,
      launches: Object.fromEntries(Object.entries(state).map(([id, s]) => [id, {
        samples: s.samples?.length ?? 0, lastClaimAt: s.lastClaimAt ?? null, lastReinvestAt: s.lastReinvestAt ?? null,
        pendingSince: s.pendingSince ?? null, sendFailures: health.sendFailures[id]?.count ?? 0,
      }])),
    };
    res.writeHead(req.url === '/health' ? (status.ok ? 200 : 503) : 404, { 'content-type': 'application/json' });
    res.end(JSON.stringify(body, (_, v) => (typeof v === 'bigint' ? v.toString() : v)));
  });
  server.listen(healthPort, '127.0.0.1', () => log('health', { url: `http://127.0.0.1:${healthPort}/health` }));
  return server;
}

main().catch((e) => {
  log('fatal', { error: String(e?.message ?? e) });
  process.exit(1);
});
