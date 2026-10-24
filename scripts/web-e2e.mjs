// End-to-end test of the web client on a local validator: drives the real pages in Chromium with an injected
// Phantom-like wallet (signing happens here, in Node, with a throwaway key) and checks the chain after each step.
//
//   KEYPAIR=<throwaway key> scripts/local-validator.sh        # the key becomes the program's upgrade authority
//   npx vite build && npx vite preview --port 5173 --strictPort   # in another shell; a build, because the dev
//                                     server's dependency re-optimization reloads pages mid-test
//   node scripts/web-e2e.mjs --keypair <same key> [--url http://127.0.0.1:8899] [--app http://localhost:5173]
//
// Covers: Setup (initialize with the devnet preset, appoint a keeper), Create, Lobby with two wallets, settle, claims,
// buy, sell, buying out the curve, breakout into DAMM v2, claiming pool fees, the creator's fee withdrawal, and the
// revenue panel against the ledger.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { ed25519 } from '@noble/curves/ed25519';
import * as anchorModule from '@anchor-lang/core';
import {
  withProgramId, WSOL, ata, createAtaIx, dexKeysForPool, dexSwapIx, globalKeys, keysOf, launchKeys, wrapSolIxs,
} from './herdcall-client.mjs';
import { tokens } from '../src/chain/format.js';

const anchor = anchorModule.default ?? anchorModule;
const { web3 } = anchor;
const { chromium } = createRequire(import.meta.url)('playwright');
const SOL = 1_000_000_000;

const arg = (name, fallback) => {
  const i = process.argv.lastIndexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};
const url = arg('url', 'http://127.0.0.1:8899');
const app = arg('app', 'http://localhost:5173');
const dexConfig = arg('dex-config', 'AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW');
const adminPath = arg('keypair', `${homedir()}/.config/solana/id.json`).replace(/^~/, homedir());

const admin = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(adminPath, 'utf8'))));
const connection = new web3.Connection(url, 'confirmed');
const idl = JSON.parse(readFileSync(new URL('../programs/idl/herdcall.json', import.meta.url), 'utf8'));
const program = new anchor.Program(withProgramId(idl), new anchor.AnchorProvider(connection, new anchor.Wallet(admin), {}));

const step = (s) => console.log(`\n▸ ${s}`);
const ok = (s) => console.log(`  ✓ ${s}`);
function check(cond, s) {
  if (!cond) throw new Error(`✗ ${s}`);
  ok(s);
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function fund(key, sol) {
  const sig = await connection.requestAirdrop(key, sol * SOL);
  await connection.confirmTransaction(sig, 'confirmed');
}

const pages = [];

/** A browser context whose window.phantom.solana is backed by `keypair`. */
async function walletPage(browser, keypair) {
  const ctx = await browser.newContext();
  await ctx.exposeFunction('__herdSign', (b64) => {
    const sig = ed25519.sign(Buffer.from(b64, 'base64'), keypair.secretKey.slice(0, 32));
    return Buffer.from(sig).toString('base64');
  });
  await ctx.addInitScript((pubkey) => {
    const publicKey = { toString: () => pubkey, toBase58: () => pubkey };
    window.phantom = {
      solana: {
        isPhantom: true,
        publicKey,
        connect: async () => ({ publicKey }),
        disconnect: async () => {},
        on: () => {},
        removeListener: () => {},
        signTransaction: async (tx) => {
          const sig = await window.__herdSign(btoa(String.fromCharCode(...tx.serializeMessage())));
          tx.addSignature(tx.feePayer, Uint8Array.from(atob(sig), (c) => c.charCodeAt(0)));
          return tx;
        },
      },
    };
  }, keypair.publicKey.toBase58());
  const page = await ctx.newPage();
  pages.push(page);
  page.on('pageerror', (e) => console.log(`  [page error] ${e.message}`));
  if (process.env.WEB_E2E_DEBUG) page.on('console', (m) => console.log(`  [console] ${m.text()}`));
  return page;
}

async function connectWallet(page) {
  const button = page.getByRole('button', { name: 'Connect wallet' }).first();
  if (await button.isVisible().catch(() => false)) await button.click();
}

/** Clicks a button and waits for a new result line (each has its own data-result id); fails with the page's error. */
async function sendVia(page, name, { scope = page, timeout = 60_000 } = {}) {
  const seen = await page.locator('[data-result]').evaluateAll((els) => els.map((e) => e.dataset.result));
  await scope.getByRole('button', { name, exact: true }).click();
  const handle = await page.waitForFunction(
    (prev) => [...document.querySelectorAll('[data-result]')].find((e) => !prev.includes(e.dataset.result))?.dataset.result,
    seen,
    { timeout },
  );
  const fresh = page.locator(`[data-result="${await handle.jsonValue()}"]`);
  if ((await fresh.getAttribute('role')) === 'alert') throw new Error(`${name}: ${await fresh.innerText()}`);
}

async function main() {
  const browser = await chromium.launch();
  const user = web3.Keypair.generate();
  const keeper = web3.Keypair.generate();
  await fund(user.publicKey, 10);
  const a = await walletPage(browser, admin);
  const b = await walletPage(browser, user);
  const g = globalKeys();

  step('Setup: initialize from the upgrade authority');
  await a.goto(`${app}/setup`);
  await connectWallet(a);
  await a.getByText('Initialized').waitFor();
  await a.getByRole('button', { name: 'devnet' }).click();
  await a.getByLabel('Meteora DAMM v2 config for breakouts').fill(dexConfig);
  await sendVia(a, 'Initialize');
  const config = await program.account.config.fetch(g.config);
  check(config.admin.equals(admin.publicKey) && config.dexConfig.toBase58() === dexConfig, 'initialized from the page');
  await a.getByText('The keeper is still the admin').waitFor();
  ok('warns that the keeper is still the admin');

  step('Setup: appoint a separate keeper');
  await a.getByLabel('Reinvest keeper (its own key, not the admin\'s)').fill(keeper.publicKey.toBase58());
  await sendVia(a, 'Set keeper');
  check((await program.account.config.fetch(g.config)).reinvestKeeper.equals(keeper.publicKey), 'keeper set');

  step('Create a launch');
  const ticker = `WEB${Date.now() % 100000}`;
  const name = `Web Test ${ticker.slice(3)}`;
  await a.goto(`${app}/create`);
  await connectWallet(a);
  await a.getByLabel('Name').fill(name);
  await a.getByLabel('Ticker').fill(ticker.toLowerCase());
  await a.getByLabel('Metadata URI (JSON with image and description, hosted by you)').fill('https://herdcall.fun/t/web.json');
  await a.getByRole('button', { name: '15s' }).click();
  await a.getByRole('button', { name: 'Open the Lobby' }).click();
  await a.waitForURL(/\/launch\//, { timeout: 60_000 });
  const mint = new web3.PublicKey(a.url().split('/launch/')[1]);
  let l = await program.account.launch.fetch(launchKeys(mint, 'X', 'x').launch);
  const k = keysOf(l);
  check(l.ticker === ticker && l.name === name && 'lobby' in l.phase, `launch ${ticker} created, page at /launch/${mint.toBase58().slice(0, 8)}…`);

  step('Lobby: two wallets deposit');
  await a.getByLabel('Amount (SOL)').fill('0.05');
  await sendVia(a, 'Answer the call');
  await b.goto(`${app}/launch/${mint.toBase58()}`);
  await connectWallet(b);
  await b.getByLabel('Amount (SOL)').fill('0.07');
  await sendVia(b, 'Answer the call');
  l = await program.account.launch.fetch(k.launch);
  check(l.depositors === 2 && l.totalDeposits.toNumber() === 0.12 * SOL, '0.12 SOL from 2 wallets');

  step('Settle');
  while (Math.floor(Date.now() / 1000) <= l.closeAt.toNumber() + 1) await sleep(1000);
  await b.reload();
  await sendVia(b, 'Ring the bell (settle)');
  l = await program.account.launch.fetch(k.launch);
  check('curve' in l.phase, 'curve open (settled from the page by a non-creator)');

  step('Claims');
  for (const p of [a, b]) {
    await p.reload();
    await sendVia(p, 'Claim');
  }
  const balance = async (key) => BigInt((await connection.getTokenAccountBalance(ata(key, mint))).value.amount);
  const lobbyA = await balance(admin.publicKey);
  const lobbyB = await balance(user.publicKey);
  check(lobbyA > 0n && lobbyB > lobbyA && lobbyA + lobbyB <= BigInt(l.lobbyTokens.toString()), 'both claimed, pro rata');

  step('Buy and sell on the curve');
  await b.getByLabel('You pay (SOL)').fill('0.2');
  const shown = (await b.getByText('You get', { exact: true }).locator('..').innerText()).replace(/\s+/g, ' ');
  await sendVia(b, `Buy $${ticker}`);
  const bought = (await balance(user.publicKey)) - lobbyB;
  check(bought > 0n && shown.includes(tokens(bought)), `bought ${tokens(bought)} tokens, as the page quoted (${shown})`);
  await b.getByRole('button', { name: 'sell' }).click();
  await b.getByLabel(`You sell ($${ticker})`).fill('1000');
  await sendVia(b, `Sell $${ticker}`);
  check((await balance(user.publicKey)) === lobbyB + bought - 1_000_000_000n, 'sold 1,000 tokens');

  step('Buy out the curve');
  await b.getByRole('button', { name: 'buy' }).click();
  await b.getByLabel('You pay (SOL)').fill('4');
  await b.getByText('Refunded (curve end)').waitFor();
  await sendVia(b, `Buy $${ticker}`);
  l = await program.account.launch.fetch(k.launch);
  check('breakingOut' in l.phase, `curve sold out, reserve ${l.reserve.toNumber() / SOL} SOL`);

  step('Breakout into Meteora DAMM v2');
  await b.reload();
  await sendVia(b, 'Break out', { timeout: 90_000 });
  l = await program.account.launch.fetch(k.launch);
  check('migrated' in l.phase && !l.dexPool.equals(web3.PublicKey.default), `pool ${l.dexPool.toBase58()}`);
  await b.getByText('On Meteora DAMM v2').waitFor();
  ok('page shows the pool');

  step('Pool volume, then claim the pool fees from the page');
  const d = dexKeysForPool(l.dexPool, k);
  const swapIn = Math.round(l.lpNative.toNumber() / 10);
  const swap = new web3.Transaction().add(
    createAtaIx(admin.publicKey, admin.publicKey, WSOL),
    ...wrapSolIxs(admin.publicKey, swapIn),
    dexSwapIx(admin.publicKey, k, { pool: d.pool }, swapIn, true),
  );
  await web3.sendAndConfirmTransaction(connection, swap, [admin]);
  await b.reload();
  await sendVia(b, 'Claim pool fees');
  l = await program.account.launch.fetch(k.launch);
  check(l.lpFeesClaimed.toNumber() > 0, `claimed ${l.lpFeesClaimed.toNumber() / SOL} SOL of pool fees`);

  step('Creator withdraws fees');
  await a.reload();
  const rent = await connection.getMinimumBalanceForRentExemption(9);
  const before = await connection.getBalance(k.creatorFees);
  check(before > rent, `creator fees ${(before - rent) / SOL} SOL`);
  await sendVia(a, 'Withdraw fees');
  check((await connection.getBalance(k.creatorFees)) === rent, 'creator fee vault back to rent');

  step('Revenue panel matches the ledger');
  await a.goto(`${app}/setup`);
  await a.getByText('Protocol revenue').waitFor();
  const r = await program.account.revenueLedger.fetch(g.revenue);
  const total = BigInt(r.curve.toString()) + BigInt(r.stampede.toString()) + BigInt(r.lp.toString());
  const text = await a.getByLabel('Protocol revenue').innerText();
  const shownRevenue = (Number(total) / SOL).toFixed(9).replace(/0+$/, '');
  check(text.includes(shownRevenue), `revenue ${shownRevenue} SOL shown`);
  check(!text.includes('uncounted'), 'vault equals the counters');

  await browser.close();
  console.log('\nWeb end-to-end passed.');
}

main().catch(async (e) => {
  console.error(e.logs ? `${e.message}\n${e.logs.join('\n')}` : e);
  const dir = process.env.WEB_E2E_SHOTS;
  if (dir) for (const [i, p] of pages.entries()) await p.screenshot({ path: `${dir}/web-e2e-${i}.png`, fullPage: true }).catch(() => {});
  process.exit(1);
});
