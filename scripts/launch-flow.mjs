// Runs a whole launch on a real cluster and checks every step: initialize (if needed), create launch with metadata,
// Lobby deposits, settle, claims, curve buys up to breakout, breakout into Meteora DAMM v2.
//
//   node scripts/launch-flow.mjs --url http://127.0.0.1:8899 --keypair ~/.config/solana/id.json \
//     --dex-config <DAMM v2 config> [--params example|devnet]
//
// The payer funds the other wallets, so no airdrop is needed beyond the payer's own balance.
import anchor from '@anchor-lang/core';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import {
  PROGRAM_ID, DAMM, METADATA_PROGRAM, TOKEN_PROGRAM, ATA_PROGRAM, SYSTEM, RENT,
  WSOL, ata, breakoutAccounts, canonicalTicker, claimLpFeesAccounts, createAtaIx, depositKey, dexKeys, dexSwapIx,
  globalKeys, launchKeys, nameHash, reinvestLpAccounts, REINVEST_MIN_LAMPORTS, wrapSolIxs,
} from './herdcall-client.mjs';

const { web3, BN } = anchor;
const SOL = 1_000_000_000;
const TOKEN = 1_000_000;

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};
const url = arg('url', 'https://api.devnet.solana.com');
const keypairPath = arg('keypair', `${homedir()}/.config/solana/id.json`).replace(/^~/, homedir());
const dexConfig = new web3.PublicKey(arg('dex-config'));
const preset = arg('params', 'devnet');

// Same numbers as the tests (example) or scaled down 100x on the SOL side so a faucet balance reaches breakout.
const PARAMS = {
  // `lobby` is each of the three wallets' deposit.
  example: { v0: 30 * SOL, minTotal: SOL, nameBond: SOL / 4, lobby: 1 * SOL },
  devnet: { v0: 0.3 * SOL, minTotal: 0.1 * SOL, nameBond: 0.01 * SOL, lobby: 0.05 * SOL },
}[preset];
const launchParams = {
  v0: new BN(PARAMS.v0),
  t0: new BN(1_000_000_000).mul(new BN(TOKEN)),
  totalSupply: new BN(1_000_000_000).mul(new BN(TOKEN)),
  curveSupply: new BN(800_000_000).mul(new BN(TOKEN)),
  lpSupply: new BN(200_000_000).mul(new BN(TOKEN)),
  feeBps: 100,
  minDeposit: new BN(SOL / 100),
  minTotal: new BN(PARAMS.minTotal),
  maxTotal: new BN(10_000).mul(new BN(SOL)),
  maxPerWallet: new BN(0),
  migrationFee: new BN(3 * SOL / 100),
  settleWindow: new BN(300),
  nameBond: new BN(PARAMS.nameBond),
};

const payer = web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(keypairPath, 'utf8'))));
const connection = new web3.Connection(url, 'confirmed');
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), { commitment: 'confirmed' });
const idl = JSON.parse(readFileSync(new URL('../programs/idl/herdcall.json', import.meta.url), 'utf8'));
const program = new anchor.Program(idl, provider);
const g = globalKeys();
const cu = web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });

const step = (msg) => console.log(`\n▸ ${msg}`);
const ok = (msg) => console.log(`  ✓ ${msg}`);
function check(cond, msg) {
  if (!cond) throw new Error(`check failed: ${msg}`);
  ok(msg);
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const lamports = (k) => connection.getBalance(k, 'confirmed');
const tokenBalance = async (k) => BigInt((await connection.getTokenAccountBalance(k, 'confirmed')).value.amount);

async function fund(to, amount) {
  const tx = new web3.Transaction().add(web3.SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports: amount }));
  await web3.sendAndConfirmTransaction(connection, tx, [payer]);
}

async function main() {
  console.log(`cluster ${url}\nprogram ${PROGRAM_ID.toBase58()}\npayer ${payer.publicKey.toBase58()} (${(await lamports(payer.publicKey)) / SOL} SOL)\nparams ${preset}`);
  check((await connection.getAccountInfo(PROGRAM_ID))?.executable, 'HerdCall program is deployed');
  check((await connection.getAccountInfo(DAMM))?.executable, 'Meteora DAMM v2 is deployed');
  check((await connection.getAccountInfo(METADATA_PROGRAM))?.executable, 'Metaplex Token Metadata is deployed');
  check((await connection.getAccountInfo(dexConfig))?.owner.equals(DAMM), 'DAMM config exists');

  step('initialize');
  if (await connection.getAccountInfo(g.config)) {
    const c = await program.account.config.fetch(g.config);
    ok(`already initialized (dex config ${c.dexConfig.toBase58()})`);
    if (!c.dexConfig.equals(dexConfig)) throw new Error('config points to another DAMM config; use set_dex_config');
  } else {
    await program.methods.initialize(launchParams, dexConfig)
      .accountsStrict({ admin: payer.publicKey, config: g.config, protocol: g.protocol, rektBack: g.rektBack, systemProgram: SYSTEM })
      .rpc();
    ok('config, protocol and Rekt-back vaults created');
  }
  const params = (await program.account.config.fetch(g.config)).params;

  step('create launch');
  const ticker = canonicalTicker(`HC${Date.now() % 100_000_000}`);
  const name = `HerdCall Test ${ticker.slice(2)}`;
  const uri = 'https://herdcall.fun/t/test.json';
  const mint = web3.Keypair.generate();
  const k = launchKeys(mint.publicKey, ticker, name);
  await program.methods.createLaunch(true, new BN(20), ticker, name, nameHash(name), uri)
    .accountsStrict({
      creator: payer.publicKey, config: g.config, mint: k.mint, launch: k.launch, tokenVault: k.tokenVault,
      solVault: k.solVault, creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow, tickerRecord: k.tickerRecord,
      nameRecord: k.nameRecord, metadata: k.metadata, tokenMetadataProgram: METADATA_PROGRAM, tokenProgram: TOKEN_PROGRAM,
      associatedTokenProgram: ATA_PROGRAM, systemProgram: SYSTEM, rent: RENT,
    })
    .signers([mint])
    .rpc();
  let l = await program.account.launch.fetch(k.launch);
  check(l.ticker === ticker && l.name === name, `launch ${k.launch.toBase58()} (${ticker}, "${name}")`);
  check((await connection.getAccountInfo(k.metadata))?.owner.equals(METADATA_PROGRAM), `metadata ${k.metadata.toBase58()}`);
  check((await tokenBalance(k.tokenVault)) === BigInt(params.totalSupply.toString()), 'whole supply in the launch vault');

  step('Lobby');
  const wallets = [payer, web3.Keypair.generate(), web3.Keypair.generate()];
  for (const w of wallets.slice(1)) await fund(w.publicKey, PARAMS.lobby + 0.05 * SOL);
  for (const w of wallets) {
    await program.methods.deposit(new BN(PARAMS.lobby))
      .accountsStrict({ user: w.publicKey, launch: k.launch, solVault: k.solVault, deposit: depositKey(k.launch, w.publicKey), systemProgram: SYSTEM })
      .signers(w === payer ? [] : [w])
      .rpc();
  }
  l = await program.account.launch.fetch(k.launch);
  check(l.depositors === 3, `3 deposits, ${l.totalDeposits.toNumber() / SOL} SOL`);

  step('settle');
  const closeAt = l.closeAt.toNumber();
  while ((await connection.getBlockTime(await connection.getSlot('confirmed'))) <= closeAt) await sleep(2_000);
  const creatorBefore = await lamports(payer.publicKey);
  await program.methods.settle()
    .accountsStrict({
      launch: k.launch, solVault: k.solVault, creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow,
      rektBack: g.rektBack, protocol: g.protocol, tickerRecord: k.tickerRecord, nameRecord: k.nameRecord, creator: payer.publicKey,
    })
    .rpc();
  l = await program.account.launch.fetch(k.launch);
  check('curve' in l.phase, `curve open, Lobby got ${l.lobbyTokens.toNumber() / TOKEN} tokens`);
  check((await lamports(payer.publicKey)) > creatorBefore, 'name bond back to the creator');
  const rec = await program.account.nameRecord.fetch(k.tickerRecord);
  check(rec.permanent, 'name and ticker permanent');

  step('claims');
  for (const w of wallets) {
    await program.methods.claim()
      .accountsStrict({
        user: w.publicKey, launch: k.launch, mint: k.mint, deposit: depositKey(k.launch, w.publicKey), owner: w.publicKey,
        solVault: k.solVault, tokenVault: k.tokenVault, userAta: ata(w.publicKey, k.mint), tokenProgram: TOKEN_PROGRAM,
        associatedTokenProgram: ATA_PROGRAM, systemProgram: SYSTEM,
      })
      .signers(w === payer ? [] : [w])
      .rpc();
  }
  ok('every depositor claimed');

  step('buy the rest of the curve');
  const trade = {
    trader: payer.publicKey, launch: k.launch, mint: k.mint, solVault: k.solVault, tokenVault: k.tokenVault,
    traderAta: ata(payer.publicKey, k.mint), creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow,
    rektBack: g.rektBack, protocol: g.protocol, interface: null, tokenProgram: TOKEN_PROGRAM,
    associatedTokenProgram: ATA_PROGRAM, systemProgram: SYSTEM,
  };
  const max = new BN('18446744073709551615');
  await program.methods.buyExactOut(max, max).accountsStrict(trade).rpc();
  l = await program.account.launch.fetch(k.launch);
  check('breakingOut' in l.phase, `curve sold out, reserve ${l.reserve.toNumber() / SOL} SOL`);

  step('breakout into Meteora DAMM v2');
  const d = dexKeys(dexConfig, k);
  const sig = await program.methods.breakout()
    .accountsStrict(breakoutAccounts(payer.publicKey, k, dexConfig))
    .preInstructions([cu])
    .rpc();
  ok(`tx ${sig}`);
  l = await program.account.launch.fetch(k.launch);
  check('migrated' in l.phase && l.dexPool.equals(d.pool), `pool ${d.pool.toBase58()}`);
  check((await tokenBalance(d.poolTokenVault)) === BigInt(l.lpTokens.toString()), `${l.lpTokens.toNumber() / TOKEN} tokens in the pool`);
  check((await tokenBalance(d.poolWsolVault)) === BigInt(l.lpNative.toString()), `${l.lpNative.toNumber() / SOL} SOL in the pool`);
  const position = (await connection.getAccountInfo(d.position)).data;
  const locked = position.readBigUInt64LE(184) + (position.readBigUInt64LE(192) << 64n);
  check(locked === BigInt(l.lpLiquidity.toString()) && locked > 0n, 'whole position permanently locked');
  const nft = (await connection.getAccountInfo(d.positionNftAccount)).data;
  check(new web3.PublicKey(nft.subarray(32, 64)).equals(k.launch), 'position NFT held by the launch PDA');

  step('trade on the pool, then claim the locked position\'s fees');
  const swapIn = Math.round(Number(l.lpNative.toString()) / 20);
  const swapTx = new web3.Transaction().add(
    createAtaIx(payer.publicKey, payer.publicKey, WSOL),
    ...wrapSolIxs(payer.publicKey, swapIn),
    dexSwapIx(payer.publicKey, k, dexConfig, swapIn, true),
  );
  await web3.sendAndConfirmTransaction(connection, swapTx, [payer]);
  ok(`bought tokens with ${swapIn / SOL} SOL on the pool`);
  const vaults = [k.creatorFees, g.rektBack, g.protocol];
  const before = await Promise.all(vaults.map(lamports));
  await program.methods.claimLpFees().accountsStrict(claimLpFeesAccounts(payer.publicKey, k, dexConfig)).rpc();
  l = await program.account.launch.fetch(k.launch);
  const after = await Promise.all(vaults.map(lamports));
  const paid = after.reduce((s, v, i) => s + (v - before[i]), 0);
  check(l.lpFeesClaimed.toNumber() > 0, `claimed ${l.lpFeesClaimed.toNumber() / SOL} SOL of pool fees`);
  check(paid + l.lpReinvestPending.toNumber() === l.lpFeesClaimed.toNumber(),
    `creator, Rekt-back and protocol paid; ${l.lpReinvestPending.toNumber() / SOL} SOL held for liquidity`);

  step('reinvest the "back to liquidity" share');
  const reinvest = program.methods.reinvestLp().accountsStrict(reinvestLpAccounts(payer.publicKey, k, dexConfig)).preInstructions([cu]);
  if (l.lpReinvestPending.toNumber() < REINVEST_MIN_LAMPORTS) {
    const err = await reinvest.rpc().then(() => null, (e) => e);
    check(err && String(err).includes('NothingToReinvest'), `below 0.01 SOL: rejected, keeps waiting`);
  } else {
    const liquidityBefore = BigInt(l.lpLiquidity.toString());
    await reinvest.rpc();
    l = await program.account.launch.fetch(k.launch);
    const lockedNow = (await connection.getAccountInfo(d.position)).data;
    const lockedAfter = lockedNow.readBigUInt64LE(184) + (lockedNow.readBigUInt64LE(192) << 64n);
    check(BigInt(l.lpLiquidity.toString()) > liquidityBefore && lockedAfter === BigInt(l.lpLiquidity.toString()),
      `${l.lpReinvested.toNumber() / SOL} SOL added to the position and locked; ${l.lpReinvestPending.toNumber() / SOL} SOL waits`);
  }
  console.log('\nAll steps passed.');
}

main().catch((e) => {
  console.error(e.logs ? `${e.message}\n${e.logs.join('\n')}` : e);
  process.exit(1);
});
