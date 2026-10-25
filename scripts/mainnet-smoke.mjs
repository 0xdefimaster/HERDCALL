// The smallest full HerdCall run, for a controlled mainnet rehearsal, and its local rehearsal.
//
// Runs where the test wallet's key is: on its owner's computer, with a keypair file of a separate test wallet (never the
// main wallet). Nothing in this repository or in a cloud container holds that key.
//
// One wallet (admin, creator, depositor, trader) and, for the full stage, a separate keeper key. Launches are unbonded
// by default (`--bonded` for bonded): the program has no instruction yet that releases a bonded creator's escrow, so a
// bonded launch would leave half of the creator fees locked. Steps: initialize with the test-scale
// preset, appoint the keeper, create the launch, one 0.1 SOL Lobby deposit, settle, claim, a small buy and sell, buy out
// the curve, breakout into DAMM v2 (whole position locked), round trips on the pool so fees accrue, claim the pool fees,
// the keeper service reinvests the liquidity share, sell the tokens back, withdraw the creator fees. Every lamport the
// two keys spend is recorded per step.
//
// Stages: `--stage lobby` stops after the curve trades (no DAMM config needed: initialize records it as unset, the
// System Program address, which breakout refuses); `--stage full` (default) goes through breakout and the keeper. A
// stage on a program already set up skips initialize, points it at --dex-config if needed and uses a new launch.
// `--sweep-to <address>` sends what is left on the wallet and the keeper to that address at the end.
// `--close-program` closes the program at the end (its rent back to the wallet; permanent). `--funded-sol <x>`: what was
// sent to the wallet before the deploy, so the report reconciles from the funding down to the final balance.
// `--upgrade-authority-to <address>` hands the program's upgrade authority to that address right after initialize (so
// its rent can be recovered with `solana program close` from a wallet outside this machine).
//
//   Rehearsal (local validator, mainnet programs): node scripts/mainnet-smoke.mjs --keypair <upgrade authority>
//   Plan only (no transaction):                    node scripts/mainnet-smoke.mjs --plan --url <mainnet rpc> --keypair <w> [--keeper <k>] --dex-config <c|none>
//   Mainnet (after approval):                      ... --live --approve <token printed by --plan>
// With a program id other than the default, set HERDCALL_PROGRAM_ID to it for the build and for this script.
//
// --live refuses to run unless the RPC's genesis hash is mainnet's, the deployed program's bytes hash to the given
// binary (--so), and --approve matches the plan's token: a hash of the wallet, keeper, program, binary, launch mode,
// DAMM config, addresses and the maximum it may spend. The program itself is deployed before, with
// `solana program deploy` (its rent is in the plan; it comes back with `solana program close`).
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import * as anchorModule from '@anchor-lang/core';
import {
  PROGRAM_ID, WSOL, adminAccounts, withProgramId, ata, breakoutAccounts, claimAccounts, claimLpFeesAccounts, createAtaIx,
  createLaunchAccounts, depositAccounts, dexSwapIx, globalKeys, initializeAccounts, launchKeys, launchParams, nameHash,
  readPool, settleAccounts, tradeAccounts, withdrawCreatorFeesAccounts, wrapSolIxs,
} from './herdcall-client.mjs';
import { marketOf, withSlippage, withSlippageUp } from '../src/chain/quote.js';

const anchor = anchorModule.default ?? anchorModule;
const { web3, BN } = anchor;
const SOL = 1_000_000_000;
const MAINNET_GENESIS = '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d';
const UPGRADEABLE_LOADER = new web3.PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');

const arg = (name, fallback) => {
  const i = process.argv.lastIndexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};
const flag = (name) => process.argv.includes(`--${name}`);
const path = (p) => p.replace(/^~/, homedir());
const live = flag('live');
const planOnly = flag('plan');
const url = arg('url', 'http://127.0.0.1:8899');
const dexArg = arg('dex-config', 'AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW');
const dexConfig = dexArg === 'none' ? web3.PublicKey.default : new web3.PublicKey(dexArg);
const stage = arg('stage', 'full');
if (!['lobby', 'full'].includes(stage)) throw new Error('--stage lobby|full');
if (stage === 'full' && dexConfig.equals(web3.PublicKey.default)) throw new Error('--stage full needs a DAMM config');
const sweepTo = arg('sweep-to', null) && new web3.PublicKey(arg('sweep-to'));
const authorityTo = arg('upgrade-authority-to', null) && new web3.PublicKey(arg('upgrade-authority-to'));
const closeProgram = flag('close-program');
if (closeProgram && authorityTo) throw new Error('--close-program and --upgrade-authority-to: the wallet can close only while it is the authority');
const fundedSol = arg('funded-sol', null) === null ? null : Math.round(Number(arg('funded-sol')) * SOL);
const wallet = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path(arg('keypair', '~/.config/solana/id.json')), 'utf8'))));
const keeperPath = arg('keeper', null);
if (stage === 'full' && !keeperPath) throw new Error('--stage full needs --keeper <keypair> (its own key, not the wallet)');
const bonded = flag('bonded');
const keeper = keeperPath ? web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path(keeperPath), 'utf8')))) : web3.Keypair.generate();
const ticker = arg('ticker', stage === 'lobby' ? 'HCSMOKE1' : 'HCSMOKE2');
const name = arg('name', `HerdCall Smoke Test ${stage === 'lobby' ? 1 : 2}`);
const uri = arg('uri', '');
const priority = Number(arg('priority-micro-lamports', live ? '20000' : '0'));
const so = arg('so', new URL('../programs/target/deploy/herdcall.so', import.meta.url).pathname);

// The test sizes. Lobby at the minimum; pool round trips large enough that the liquidity share reaches the 0.01 SOL
// reinvest minimum.
const LOBBY = 0.1 * SOL;
const PROBE_BUY = 0.01 * SOL;
const KEEPER_FUND = 0.06 * SOL;
const ROUND_TRIP = 0.5 * SOL;
const ROUND_TRIPS = Number(arg('round-trips', '7'));
// Ceiling the plan approves: the most the wallet may be down at any point of the run (measured peak in the rehearsal:
// 0.145 SOL for the lobby stage, 3.07 SOL for the full one). The run stops if it is reached, and does not start on a
// wallet that holds less.
const MAX_SPEND = BigInt(Math.round(Number(arg('max-spend-sol', stage === 'lobby' ? '0.25' : '3.3')) * SOL));

const connection = new web3.Connection(url, 'confirmed');
const program = new anchor.Program(
  withProgramId(JSON.parse(readFileSync(new URL('../programs/idl/herdcall.json', import.meta.url), 'utf8'))),
  new anchor.AnchorProvider(connection, new anchor.Wallet(wallet), { commitment: 'confirmed' }),
);
const g = globalKeys();
const mint = web3.Keypair.generate();
const k = launchKeys(mint.publicKey, ticker, name);

function approvalToken() {
  const plan = {
    stage, bonded, wallet: wallet.publicKey.toBase58(), keeper: keeperPath ? keeper.publicKey.toBase58() : null,
    program: PROGRAM_ID.toBase58(), soSha256: createHash('sha256').update(readFileSync(so)).digest('hex'),
    dexConfig: dexConfig.toBase58(), ticker, name, maxSpend: MAX_SPEND.toString(), sweepTo: sweepTo?.toBase58() ?? null,
    upgradeAuthorityTo: authorityTo?.toBase58() ?? null, closeProgram,
  };
  return { plan, token: createHash('sha256').update(JSON.stringify(plan)).digest('hex').slice(0, 16) };
}

const rows = [];
async function balances() {
  const [w, kp] = await Promise.all([connection.getBalance(wallet.publicKey), connection.getBalance(keeper.publicKey)]);
  return { w, k: kp };
}
/** Runs one step and records what the wallet and the keeper spent (negative: got back). */
let fundedAt = null;
/** `expect`: lamports the step is known to spend; checked against the ceiling before anything is sent. */
async function step(label, fn, note = '', expect = 0) {
  const before = await balances();
  if (fundedAt !== null && BigInt(fundedAt - before.w + Number(expect)) > MAX_SPEND) {
    throw new Error(`not sent: ${label} would take the wallet ${(fundedAt - before.w + Number(expect)) / SOL} SOL down, above the approved ${Number(MAX_SPEND) / SOL}`);
  }
  const out = await fn();
  const after = await balances();
  if (fundedAt !== null && BigInt(fundedAt - after.w) > MAX_SPEND) {
    throw new Error(`stopped: the wallet is down ${(fundedAt - after.w) / SOL} SOL, above the approved ${Number(MAX_SPEND) / SOL}`);
  }
  rows.push({ label, wallet: before.w - after.w, keeper: before.k - after.k, note: typeof note === 'function' ? note(out) : note });
  const r = rows[rows.length - 1];
  console.log(`  ✓ ${label.padEnd(34)} wallet ${(r.wallet / SOL).toFixed(6).padStart(10)}  keeper ${(r.keeper / SOL).toFixed(6).padStart(9)}  ${r.note}`);
  return out;
}

const budget = () => [
  web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
  ...(priority > 0 ? [web3.ComputeBudgetProgram.setComputeUnitPrice({ microLamports: priority })] : []),
];
const rpc = (builder, signers = []) => builder.preInstructions(budget()).signers(signers).rpc();
const sendTx = (ixs, signers) => web3.sendAndConfirmTransaction(connection, new web3.Transaction().add(...budget(), ...ixs), signers);
const tokenBalance = async (owner) => BigInt((await connection.getTokenAccountBalance(ata(owner, mint.publicKey))).value.amount);

function runKeeper(secs) {
  const dir = mkdtempSync(join(tmpdir(), 'herdcall-smoke-'));
  const keyFile = join(dir, 'keeper.json');
  writeFileSync(keyFile, JSON.stringify(Array.from(keeper.secretKey)));
  return new Promise((resolve, reject) => {
    const p = spawn(process.execPath, [
      new URL('./keeper/keeper.mjs', import.meta.url).pathname, '--url', url, '--keypair', keyFile, '--state', join(dir, 'state.json'),
      // A short price window for the test: 10 samples over 50 s (production: 30 minutes).
      '--sample-secs', '5', '--window-secs', '60', '--min-samples', '10', '--claim-every-secs', '100000', '--reinvest-every-secs', '10',
      '--priority-micro-lamports', String(priority), '--run-for-secs', String(secs),
    ], { env: process.env });
    let out = '';
    p.stdout.on('data', (d) => { out += d; });
    p.stderr.on('data', (d) => { out += d; });
    p.on('exit', (code) => (code === 0 ? resolve(out.split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l))) : reject(new Error(out))));
  });
}

async function main() {
  const genesis = await connection.getGenesisHash();
  const onMainnet = genesis === MAINNET_GENESIS;
  const { plan, token } = approvalToken();
  const programInfo = await connection.getAccountInfo(PROGRAM_ID);
  const soSize = statSync(so).size;
  const programRent = await connection.getMinimumBalanceForRentExemption(soSize + 45);
  const programAccountRent = await connection.getMinimumBalanceForRentExemption(36);
  const writeTxs = Math.ceil(soSize / 1000);

  if (planOnly) {
    console.log(JSON.stringify({ cluster: onMainnet ? 'mainnet' : genesis, ...plan, programDeployed: Boolean(programInfo?.executable),
      deploy: { soBytes: soSize, programDataRentLamports: programRent, programAccountRentLamports: programAccountRent, writeTransactions: writeTxs, writeFeesLamports: writeTxs * 5000 },
      approve: token }, null, 2));
    return;
  }
  if (live) {
    if (!onMainnet) throw new Error(`--live needs a mainnet RPC (genesis ${genesis})`);
    if (arg('approve') !== token) throw new Error('--approve does not match this plan: run --plan and get it approved');
  } else if (onMainnet) {
    throw new Error('this RPC is mainnet: a rehearsal runs only on a local validator; use --plan, then --live with an approval');
  }
  if (!programInfo?.executable) throw new Error(`program ${PROGRAM_ID.toBase58()} is not deployed on ${url}`);
  // The deployed bytes must be the binary the plan names: program data = 45 bytes of header, the ELF, zero padding.
  const programData = web3.PublicKey.findProgramAddressSync([PROGRAM_ID.toBuffer()], UPGRADEABLE_LOADER)[0];
  const deployed = (await connection.getAccountInfo(programData))?.data;
  const elf = deployed?.subarray(45, 45 + soSize);
  if (!elf || createHash('sha256').update(elf).digest('hex') !== plan.soSha256 || deployed.subarray(45 + soSize).some((b) => b !== 0)) {
    throw new Error(`the deployed program is not ${so} (sha256 ${plan.soSha256})`);
  }
  console.log(`deployed program matches ${so} (sha256 ${plan.soSha256})`);
  const start = await balances();
  if (!live && fundedSol === null && start.w < 10 * SOL) {
    await connection.confirmTransaction(await connection.requestAirdrop(wallet.publicKey, 20 * SOL), 'confirmed');
  }
  const funded = await balances();
  if (BigInt(funded.w) < MAX_SPEND) throw new Error(`the wallet holds ${funded.w / SOL} SOL, less than the run may need (${Number(MAX_SPEND) / SOL})`);
  fundedAt = funded.w;
  console.log(`${live ? 'MAINNET' : 'rehearsal'}: wallet ${wallet.publicKey.toBase58()} (${funded.w / SOL} SOL), keeper ${keeper.publicKey.toBase58()}`);
  console.log(`program ${PROGRAM_ID.toBase58()}, DAMM config ${dexConfig.toBase58()}, launch ${ticker} "${name}", mint ${mint.publicKey.toBase58()}\n`);
  const lowest = { w: funded.w };
  const track = async () => { lowest.w = Math.min(lowest.w, (await balances()).w); };

  const existing = await program.account.config.fetchNullable(g.config);
  if (!existing) {
    const p = Object.fromEntries(Object.entries(launchParams('devnet')).map(([key, v]) => [key, typeof v === 'bigint' ? new BN(v.toString()) : v]));
    await step('initialize (test-scale preset)', () => rpc(program.methods.initialize(p, dexConfig).accountsStrict(initializeAccounts(wallet.publicKey))),
      dexConfig.equals(web3.PublicKey.default) ? 'DAMM config unset' : 'config, vaults, ledger rent');
  } else {
    if (!existing.admin.equals(wallet.publicKey)) throw new Error(`initialized with another admin (${existing.admin.toBase58()})`);
    if (stage === 'full' && !existing.dexConfig.equals(dexConfig)) {
      await step('set_dex_config', () => rpc(program.methods.setDexConfig(dexConfig).accountsStrict(adminAccounts(wallet.publicKey))), dexConfig.toBase58());
    }
  }
  if (authorityTo) {
    const programData = web3.PublicKey.findProgramAddressSync([PROGRAM_ID.toBuffer()], UPGRADEABLE_LOADER)[0];
    const current = (await connection.getAccountInfo(programData)).data;
    if (current[12] === 1 && new web3.PublicKey(current.subarray(13, 45)).equals(wallet.publicKey)) {
      // Upgradeable loader SetAuthority (4): program data, current authority (signer), new authority.
      const ix = new web3.TransactionInstruction({ programId: UPGRADEABLE_LOADER, data: Buffer.from([4, 0, 0, 0]), keys: [
        { pubkey: programData, isSigner: false, isWritable: true }, { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
        { pubkey: authorityTo, isSigner: false, isWritable: false }] });
      await step('hand over the upgrade authority', () => sendTx([ix], [wallet]), authorityTo.toBase58());
      const after = (await connection.getAccountInfo(programData)).data;
      if (!new web3.PublicKey(after.subarray(13, 45)).equals(authorityTo)) throw new Error('upgrade authority did not change');
    }
  }
  if (keeperPath && !(existing?.reinvestKeeper.equals(keeper.publicKey))) {
    await step('appoint the keeper', () => rpc(program.methods.setReinvestKeeper(keeper.publicKey).accountsStrict(adminAccounts(wallet.publicKey))));
  }
  if (stage === 'full' && (await connection.getBalance(keeper.publicKey)) < KEEPER_FUND) {
    await step('fund the keeper', () => sendTx([web3.SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: keeper.publicKey, lamports: KEEPER_FUND })], [wallet]),
      `${KEEPER_FUND / SOL} SOL`, KEEPER_FUND);
  }
  await step('create_launch', () => rpc(program.methods.createLaunch(bonded, new BN(15), ticker, name, nameHash(name), uri).accountsStrict(createLaunchAccounts(wallet.publicKey, k)), [mint]),
    `${bonded ? 'bonded' : 'unbonded'}: mint, metadata (Metaplex fee), records, vaults; bond held`);
  await step('deposit (Lobby)', () => rpc(program.methods.deposit(new BN(LOBBY)).accountsStrict(depositAccounts(wallet.publicKey, k))), `${LOBBY / SOL} SOL, the minimum`, LOBBY);
  await track();
  const l0 = await program.account.launch.fetch(k.launch);
  while (Math.floor(Date.now() / 1000) <= l0.closeAt.toNumber() + 1) await new Promise((r) => setTimeout(r, 1000));
  await step('settle', () => rpc(program.methods.settle().accountsStrict(settleAccounts(k, wallet.publicKey))), 'bond back');
  await step('claim', () => rpc(program.methods.claim().accountsStrict(claimAccounts(wallet.publicKey, k))), 'token account rent');
  const lobbyTokens = await tokenBalance(wallet.publicKey);

  let l = await program.account.launch.fetch(k.launch);
  const q = marketOf(l).quoteBuyExactIn(BigInt(PROBE_BUY));
  await step('buy 0.01 SOL', () => rpc(program.methods.buyExactIn(new BN(PROBE_BUY), new BN(withSlippage(q.tokens, 100).toString())).accountsStrict(tradeAccounts(wallet.publicKey, k))));
  const probe = (await tokenBalance(wallet.publicKey)) - lobbyTokens;
  l = await program.account.launch.fetch(k.launch);
  const sq = marketOf(l).quoteSell(probe);
  await step('sell it back', () => rpc(program.methods.sell(new BN(probe.toString()), new BN(withSlippage(sq.received, 100).toString())).accountsStrict(tradeAccounts(wallet.publicKey, k))), '2% of the trade in fees');

  if (stage === 'lobby') {
    l = await program.account.launch.fetch(k.launch);
    const all = await tokenBalance(wallet.publicKey);
    const back = marketOf(l).quoteSell(all);
    await step('sell the Lobby tokens to the curve', () => rpc(program.methods.sell(new BN(all.toString()), new BN(withSlippage(back.received, 100).toString())).accountsStrict(tradeAccounts(wallet.publicKey, k))),
      'the Lobby deposit back, less fees');
    await step('close the empty token account', () => sendTx([new web3.TransactionInstruction({
      programId: new web3.PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'), data: Buffer.from([9]), keys: [
        { pubkey: ata(wallet.publicKey, mint.publicKey), isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: false, isWritable: true }, { pubkey: wallet.publicKey, isSigner: true, isWritable: false }] })], [wallet]),
    'its rent back');
    await step('withdraw creator fees', () => rpc(program.methods.withdrawCreatorFees().accountsStrict(withdrawCreatorFeesAccounts(wallet.publicKey, k))), 'creator share back');
    return finish(funded, lowest, null, plan);
  }

  l = await program.account.launch.fetch(k.launch);
  const rest = marketOf(l).quoteBuyExactOut(BigInt(l.params.curveSupply.toString()));
  await step('buy out the curve', () => rpc(program.methods.buyExactOut(new BN(rest.tokens.toString()), new BN(withSlippageUp(rest.gross, 50).toString())).accountsStrict(tradeAccounts(wallet.publicKey, k))),
    `${(Number(rest.gross) / SOL).toFixed(4)} SOL into the curve`, withSlippageUp(rest.gross, 50));
  await track();
  await step('breakout', () => rpc(program.methods.breakout().accountsStrict(breakoutAccounts(wallet.publicKey, k, dexConfig))), 'fronts pool rent, gets the breakout fee');
  l = await program.account.launch.fetch(k.launch);
  console.log(`    pool ${l.dexPool.toBase58()}: ${l.lpNative.toNumber() / SOL} SOL + ${l.lpTokens.toNumber() / 1e6} tokens, locked`);

  await step(`${ROUND_TRIPS} round trips of ${ROUND_TRIP / SOL} SOL on the pool`, async () => {
    await sendTx([createAtaIx(wallet.publicKey, wallet.publicKey, WSOL), ...wrapSolIxs(wallet.publicKey, ROUND_TRIP)], [wallet]);
    for (let i = 0; i < ROUND_TRIPS; i++) {
      const before = await tokenBalance(wallet.publicKey);
      const wsol = BigInt((await connection.getTokenAccountBalance(ata(wallet.publicKey, WSOL))).value.amount);
      await sendTx([dexSwapIx(wallet.publicKey, k, { pool: l.dexPool }, wsol, true)], [wallet]);
      await sendTx([dexSwapIx(wallet.publicKey, k, { pool: l.dexPool }, (await tokenBalance(wallet.publicKey)) - before, false)], [wallet]);
    }
  }, 'pool fees: Meteora\'s cut and the locked position\'s', ROUND_TRIP);
  await track();
  await step('claim_lp_fees', () => rpc(program.methods.claimLpFees().accountsStrict(claimLpFeesAccounts(wallet.publicKey, k, { pool: l.dexPool }))), 'split at once');
  l = await program.account.launch.fetch(k.launch);
  console.log(`    claimed ${l.lpFeesClaimed.toNumber() / SOL} SOL; ${l.lpReinvestPending.toNumber() / SOL} SOL waits for the keeper (cap ${Number(readPool((await connection.getAccountInfo(l.dexPool)).data).sol / 200n) / SOL})`);

  const events = await step('keeper service reinvests', () => runKeeper(80), 'keeper pays its fees');
  l = await program.account.launch.fetch(k.launch);
  const reinvested = events.some((e) => e.event === 'reinvested');
  console.log(`    ${reinvested ? `reinvested ${l.lpReinvested.toNumber() / SOL} SOL into the locked position` : `NOT reinvested: ${JSON.stringify(events.filter((e) => e.event?.startsWith('reinvest')).slice(-1))}`}`);

  await step('sell the tokens to the pool', async () => {
    const all = await tokenBalance(wallet.publicKey);
    await sendTx([dexSwapIx(wallet.publicKey, k, { pool: l.dexPool }, all, false)], [wallet]);
    await sendTx([new web3.TransactionInstruction({ programId: new web3.PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'), keys: [
      { pubkey: ata(wallet.publicKey, WSOL), isSigner: false, isWritable: true }, { pubkey: wallet.publicKey, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false }], data: Buffer.from([9]) })], [wallet]);
  }, 'unwraps the SOL');
  await step('withdraw creator fees', () => rpc(program.methods.withdrawCreatorFees().accountsStrict(withdrawCreatorFeesAccounts(wallet.publicKey, k))), 'creator share back');
  return finish(funded, lowest, { pool: l.dexPool, reinvested }, plan);
}

async function finish(funded, lowest, dex, plan) {
  const r = await program.account.revenueLedger.fetch(g.revenue);
  const vault = async (key) => (await connection.getBalance(key)) - (await connection.getMinimumBalanceForRentExemption(9));
  const stuck = {
    protocol: await vault(g.protocol), rektBack: await vault(g.rektBack), creatorEscrow: await vault(k.creatorEscrow),
    ...(dex ? { poolSol: Number(readPool((await connection.getAccountInfo(dex.pool)).data).sol) } : { curveReserve: (await program.account.launch.fetch(k.launch)).reserve.toNumber() }),
  };
  if (closeProgram) {
    const programData = web3.PublicKey.findProgramAddressSync([PROGRAM_ID.toBuffer()], UPGRADEABLE_LOADER)[0];
    // Upgradeable loader Close (5): program data, recipient, authority (signer), program.
    const ix = new web3.TransactionInstruction({ programId: UPGRADEABLE_LOADER, data: Buffer.from([5, 0, 0, 0]), keys: [
      { pubkey: programData, isSigner: false, isWritable: true }, { pubkey: wallet.publicKey, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false }, { pubkey: PROGRAM_ID, isSigner: false, isWritable: true }] });
    await step('close the program', () => sendTx([ix], [wallet]), 'program data rent back; permanent');
  }
  const end = await balances();
  let swept = 0;
  if (sweepTo) {
    const fee = 5000 + Math.ceil((priority * 400_000) / 1e6);
    for (const kp of [keeper, wallet]) {
      const bal = await connection.getBalance(kp.publicKey);
      if (bal > fee) {
        await sendTx([web3.SystemProgram.transfer({ fromPubkey: kp.publicKey, toPubkey: sweepTo, lamports: bal - fee })], [kp]);
        swept += bal - fee;
      }
    }
  }
  const after = await balances();

  // From the funding down to the final balance: every lamport is in one line.
  const sol = (x) => (x / SOL).toFixed(9);
  const lines = [];
  const add = (label, lamports) => lines.push({ label, lamports });
  const start = fundedSol ?? funded.w;
  add(fundedSol === null ? 'wallet at the start' : 'sent to the wallet (--funded-sol)', start);
  if (fundedSol !== null) add('deploy (rent, program account, write fees)', -(fundedSol - funded.w));
  for (const row of rows) add(row.label, -(row.wallet + row.keeper));
  if (sweepTo) add(`swept to ${sweepTo.toBase58()}`, -swept);
  const sum = lines.reduce((t, l) => t + l.lamports, 0);
  const left = after.w + after.k;
  const reconciliation = {
    lines, leftInWalletAndKeeper: left, balanced: sum === left,
    lossLamports: start - left - swept,
  };
  console.log('\nReconciliation (SOL):');
  for (const l of lines) console.log(`  ${l.label.padEnd(50)} ${sol(l.lamports).padStart(14)}`);
  console.log(`  ${'= left on the wallet and the keeper'.padEnd(50)} ${sol(left).padStart(14)}${sum === left ? '' : `  (lines sum to ${sol(sum)})`}`);
  console.log(`  ${'loss (sent - left - swept)'.padEnd(50)} ${sol(reconciliation.lossLamports).padStart(14)}`);
  const summary = {
    mode: live ? 'mainnet' : 'rehearsal',
    peakWalletNeedLamports: funded.w - lowest.w,
    revenue: { curve: r.curve.toString(), stampede: r.stampede.toString(), lp: r.lp.toString(), rounding: r.rounding.toString(), other: r.other.toString() },
    leftOnChain: stuck,
    reinvested: dex?.reinvested ?? null,
  };
  console.log(`\n${JSON.stringify(summary, null, 2)}`);
  if (!reconciliation.balanced) throw new Error('the reconciliation does not add up');
  if (arg('report')) writeFileSync(arg('report'), JSON.stringify({ rows, reconciliation, summary, plan }, null, 2));
}

main().catch((e) => {
  console.error(e.logs ? `${e.message}\n${e.logs.join('\n')}` : e);
  process.exit(1);
});
