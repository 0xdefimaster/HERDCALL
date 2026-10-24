// Addresses and argument encoding of the HerdCall program, shared by scripts (and later the web client).
// Mirrors programs/herdcall/src/state.rs and the LiteSVM harness in programs/herdcall-tests/tests/common/mod.rs.
import * as anchorModule from '@anchor-lang/core';
import { sha256 } from '@noble/hashes/sha256';

// Node loads anchor's CommonJS build (a default export), the browser its ESM build (named exports only).
const anchor = anchorModule.default ?? anchorModule;
const { web3 } = anchor;
const { PublicKey } = web3;

export const PROGRAM_ID = new PublicKey('EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K');
export const DAMM = new PublicKey('cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG');
export const DAMM_POOL_AUTHORITY = new PublicKey('HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC');
export const METADATA_PROGRAM = new PublicKey('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');
export const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const WSOL = new PublicKey('So11111111111111111111111111111111111111112');
export const SYSTEM = web3.SystemProgram.programId;
export const RENT = web3.SYSVAR_RENT_PUBKEY;
export const UPGRADEABLE_LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');

const enc = (s) => new TextEncoder().encode(s);
const pda = (seeds, program = PROGRAM_ID) => PublicKey.findProgramAddressSync(seeds, program)[0];

export const ata = (owner, mint) => pda([owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()], ATA_PROGRAM);

/** Ticker as the program stores it: upper case, 1–10 characters of A–Z and 0–9. `null` if not valid. */
export function canonicalTicker(input) {
  const t = input.trim().toUpperCase();
  return /^[A-Z0-9]{1,10}$/.test(t) ? t : null;
}

/**
 * Name as it is compared: surrounding whitespace removed, lower case. `null` if longer than 32 bytes, empty or with a
 * control character (herdcall_core::names::canonical_name). The program re-checks; a mismatch only fails the call.
 */
export function canonicalName(name) {
  const t = name.trim();
  if (enc(name).length > 32 || t.length === 0 || /\p{Cc}/u.test(t)) return null;
  return t.toLowerCase();
}

export function nameHash(name) {
  const c = canonicalName(name);
  if (c === null) throw new Error(`invalid name: ${JSON.stringify(name)}`);
  return Array.from(sha256(enc(c)));
}

export const globalKeys = () => ({
  config: pda([enc('config')]),
  protocol: pda([enc('protocol')]),
  rektBack: pda([enc('rekt_back')]),
  revenue: pda([enc('revenue')]),
  migrator: pda([enc('migrator')]),
});

/** The program's ProgramData account: `initialize` checks that its signer is the upgrade authority recorded there. */
export const programDataKey = () => pda([PROGRAM_ID.toBuffer()], UPGRADEABLE_LOADER);

/** Accounts of `initialize` for anchor's accountsStrict. `admin` must be the program's upgrade authority. */
export function initializeAccounts(admin) {
  const g = globalKeys();
  return {
    admin, config: g.config, protocol: g.protocol, rektBack: g.rektBack, revenue: g.revenue,
    program: PROGRAM_ID, programData: programDataKey(), systemProgram: SYSTEM,
  };
}

/** Every account that belongs to one launch. */
export function launchKeys(mint, ticker, name) {
  const launch = pda([enc('launch'), mint.toBuffer()]);
  const lk = launch.toBuffer();
  return {
    mint,
    launch,
    tokenVault: ata(launch, mint),
    solVault: pda([enc('sol_vault'), lk]),
    creatorFees: pda([enc('creator_fees'), lk]),
    creatorEscrow: pda([enc('creator_escrow'), lk]),
    tickerRecord: pda([enc('ticker'), enc(ticker)]),
    nameRecord: pda([enc('name'), Uint8Array.from(nameHash(name))]),
    metadata: pda([enc('metadata'), METADATA_PROGRAM.toBuffer(), mint.toBuffer()], METADATA_PROGRAM),
    migratorToken: pda([enc('migrator_token'), lk]),
    migratorWsol: pda([enc('migrator_wsol'), lk]),
    positionNftMint: pda([enc('position_nft'), lk]),
  };
}

export const depositKey = (launch, user) => pda([enc('deposit'), launch.toBuffer(), user.toBuffer()]);
export const interfaceKey = (owner) => pda([enc('interface'), owner.toBuffer()]);

/** Keys of a launch from its on-chain account (`program.account.launch.fetch`). */
export const keysOf = (launch) => launchKeys(launch.mint, launch.ticker, launch.name);

/** Accounts of `create_launch` for anchor's accountsStrict. The mint is a fresh keypair that signs too. */
export function createLaunchAccounts(creator, k) {
  return {
    creator, config: globalKeys().config, mint: k.mint, launch: k.launch, tokenVault: k.tokenVault, solVault: k.solVault,
    creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow, tickerRecord: k.tickerRecord, nameRecord: k.nameRecord,
    metadata: k.metadata, tokenMetadataProgram: METADATA_PROGRAM, tokenProgram: TOKEN_PROGRAM,
    associatedTokenProgram: ATA_PROGRAM, systemProgram: SYSTEM, rent: RENT,
  };
}

export const depositAccounts = (user, k) => ({
  user, launch: k.launch, solVault: k.solVault, deposit: depositKey(k.launch, user), systemProgram: SYSTEM,
});

/** `settle` is permissionless; `creator` must be the launch's creator (it receives the name bond back). */
export function settleAccounts(k, creator) {
  const g = globalKeys();
  return {
    launch: k.launch, solVault: k.solVault, creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow,
    rektBack: g.rektBack, protocol: g.protocol, revenue: g.revenue, tickerRecord: k.tickerRecord, nameRecord: k.nameRecord,
    creator,
  };
}

export const claimAccounts = (user, k) => ({
  user, launch: k.launch, mint: k.mint, deposit: depositKey(k.launch, user), owner: user, solVault: k.solVault,
  tokenVault: k.tokenVault, userAta: ata(user, k.mint), tokenProgram: TOKEN_PROGRAM,
  associatedTokenProgram: ATA_PROGRAM, systemProgram: SYSTEM,
});

export const refundAccounts = (user, k) => ({
  user, launch: k.launch, deposit: depositKey(k.launch, user), solVault: k.solVault,
});

/** Accounts of `buy_exact_in`, `buy_exact_out` and `sell`. `iface`: a registered interface's PDA, or null. */
export function tradeAccounts(trader, k, iface = null) {
  const g = globalKeys();
  return {
    trader, launch: k.launch, mint: k.mint, solVault: k.solVault, tokenVault: k.tokenVault, traderAta: ata(trader, k.mint),
    creatorFees: k.creatorFees, creatorEscrow: k.creatorEscrow, rektBack: g.rektBack, protocol: g.protocol,
    revenue: g.revenue, interface: iface, tokenProgram: TOKEN_PROGRAM, associatedTokenProgram: ATA_PROGRAM,
    systemProgram: SYSTEM,
  };
}

export const releaseNameAccounts = (k, creator) => ({
  tickerRecord: k.tickerRecord, nameRecord: k.nameRecord, launch: k.launch, creator,
});

export const withdrawCreatorFeesAccounts = (creator, k) => ({ creator, launch: k.launch, creatorFees: k.creatorFees });

export const adminAccounts = (admin) => ({ admin, config: globalKeys().config });

const SOL_LAMPORTS = 1_000_000_000n;
const TOKEN_UNITS = 1_000_000n;

/**
 * Launch parameters for `initialize`, as decimal strings of base units (wrap each in BN for anchor).
 * - `example`: decided: 1% fee, T0 = supply = 1 billion (800 million curve, 200 million LP), minimum Lobby 1 SOL, name
 *   bond 0.25 SOL (HERDCALL.md §12, CURVE_SPEC §1). Not decided: V0 = 30 SOL is CURVE_SPEC's simulation example, and
 *   the migration fee (0.03 SOL), settle window and deposit limits are working values. Parameters are fixed at
 *   `initialize`, so a production setup needs these decided first.
 * - `devnet`: the same with the SOL side scaled down so a small balance reaches breakout. For tests only.
 */
export const LAUNCH_PRESETS = {
  example: { v0: 30n * SOL_LAMPORTS, minTotal: SOL_LAMPORTS, nameBond: SOL_LAMPORTS / 4n },
  devnet: { v0: 6n * SOL_LAMPORTS / 10n, minTotal: SOL_LAMPORTS / 10n, nameBond: SOL_LAMPORTS / 100n },
};

export function launchParams(preset) {
  const p = LAUNCH_PRESETS[preset];
  if (!p) throw new Error(`unknown preset ${preset}`);
  const supply = 1_000_000_000n * TOKEN_UNITS;
  return {
    v0: p.v0, t0: supply, totalSupply: supply, curveSupply: 800_000_000n * TOKEN_UNITS, lpSupply: 200_000_000n * TOKEN_UNITS,
    feeBps: 100, minDeposit: SOL_LAMPORTS / 100n, minTotal: p.minTotal, maxTotal: 10_000n * SOL_LAMPORTS, maxPerWallet: 0n,
    migrationFee: 3n * SOL_LAMPORTS / 100n, settleWindow: 300n, nameBond: p.nameBond,
  };
}

/** DAMM v2 accounts of a launch's pool: `["pool", config, larger mint, smaller mint]` and what hangs off it. */
export function dexKeys(dexConfig, k) {
  const [hi, lo] = Buffer.compare(k.mint.toBuffer(), WSOL.toBuffer()) > 0 ? [k.mint, WSOL] : [WSOL, k.mint];
  return dexKeysForPool(pda([enc('pool'), dexConfig.toBuffer(), hi.toBuffer(), lo.toBuffer()], DAMM), k);
}

/** Same, from the pool address a migrated launch recorded (`Launch.dex_pool`). */
export function dexKeysForPool(pool, k) {
  const nft = k.positionNftMint.toBuffer();
  return {
    pool,
    position: pda([enc('position'), nft], DAMM),
    positionNftAccount: pda([enc('position_nft_account'), nft], DAMM),
    poolTokenVault: pda([enc('token_vault'), k.mint.toBuffer(), pool.toBuffer()], DAMM),
    poolWsolVault: pda([enc('token_vault'), WSOL.toBuffer(), pool.toBuffer()], DAMM),
    eventAuthority: pda([enc('__event_authority')], DAMM),
  };
}

/** Accounts of `breakout` for anchor's accountsStrict. */
export function breakoutAccounts(keeper, k, dexConfig) {
  const g = globalKeys();
  const d = dexKeys(dexConfig, k);
  return {
    keeper,
    launch: k.launch,
    config: g.config,
    mint: k.mint,
    tokenVault: k.tokenVault,
    solVault: k.solVault,
    protocol: g.protocol,
    revenue: g.revenue,
    dexConfig,
    migrator: g.migrator,
    migratorToken: k.migratorToken,
    migratorWsol: k.migratorWsol,
    nativeMint: WSOL,
    positionNftMint: k.positionNftMint,
    positionNftAccount: d.positionNftAccount,
    pool: d.pool,
    position: d.position,
    poolTokenVault: d.poolTokenVault,
    poolWsolVault: d.poolWsolVault,
    poolAuthority: DAMM_POOL_AUTHORITY,
    dexEventAuthority: d.eventAuthority,
    dexProgram: DAMM,
    tokenProgram: TOKEN_PROGRAM,
    token2022Program: TOKEN_2022_PROGRAM,
    systemProgram: SYSTEM,
  };
}

// `dexConfig` is the DAMM config the pool was created with, or `{ pool }` with the launch's recorded pool.
const poolKeys = (x, k) => (x instanceof PublicKey ? dexKeys(x, k) : dexKeysForPool(x.pool, k));

/** Accounts of `claim_lp_fees` for anchor's accountsStrict. */
export function claimLpFeesAccounts(caller, k, dexConfig) {
  const g = globalKeys();
  const d = poolKeys(dexConfig, k);
  return {
    caller,
    launch: k.launch,
    mint: k.mint,
    nativeMint: WSOL,
    tokenVault: k.tokenVault,
    solVault: k.solVault,
    creatorFees: k.creatorFees,
    rektBack: g.rektBack,
    protocol: g.protocol,
    revenue: g.revenue,
    feeWsol: pda([enc('lp_fee_wsol'), k.launch.toBuffer()]),
    pool: d.pool,
    position: d.position,
    positionNftAccount: d.positionNftAccount,
    poolTokenVault: d.poolTokenVault,
    poolWsolVault: d.poolWsolVault,
    poolAuthority: DAMM_POOL_AUTHORITY,
    dexEventAuthority: d.eventAuthority,
    dexProgram: DAMM,
    tokenProgram: TOKEN_PROGRAM,
    systemProgram: SYSTEM,
  };
}

const SWAP = Uint8Array.from([248, 198, 158, 145, 225, 117, 135, 200]); // sha256("global:swap")[..8]

/** DAMM v2 swap on a launch's pool. `solIn` buys tokens with the trader's wrapped SOL. */
export function dexSwapIx(trader, k, dexConfig, amountIn, solIn) {
  const d = poolKeys(dexConfig, k);
  const [input, output] = solIn ? [ata(trader, WSOL), ata(trader, k.mint)] : [ata(trader, k.mint), ata(trader, WSOL)];
  const data = Buffer.alloc(24);
  data.set(SWAP, 0);
  data.writeBigUInt64LE(BigInt(amountIn), 8);
  const m = (pubkey, isWritable = false, isSigner = false) => ({ pubkey, isWritable, isSigner });
  return new web3.TransactionInstruction({
    programId: DAMM,
    keys: [
      m(DAMM_POOL_AUTHORITY), m(d.pool, true), m(input, true), m(output, true), m(d.poolTokenVault, true),
      m(d.poolWsolVault, true), m(k.mint), m(WSOL), m(trader, false, true), m(TOKEN_PROGRAM), m(TOKEN_PROGRAM),
      m(DAMM), m(d.eventAuthority), m(DAMM),
    ],
    data,
  });
}

/** Idempotent associated token account creation. */
export function createAtaIx(payer, owner, mint) {
  const m = (pubkey, isWritable = false, isSigner = false) => ({ pubkey, isWritable, isSigner });
  return new web3.TransactionInstruction({
    programId: ATA_PROGRAM,
    keys: [m(payer, true, true), m(ata(owner, mint), true), m(owner), m(mint), m(SYSTEM), m(TOKEN_PROGRAM)],
    data: Buffer.from([1]),
  });
}

/** Wraps `lamports` into the owner's wrapped SOL account (which must exist). */
export function wrapSolIxs(owner, lamports) {
  const w = ata(owner, WSOL);
  return [
    web3.SystemProgram.transfer({ fromPubkey: owner, toPubkey: w, lamports }),
    new web3.TransactionInstruction({ programId: TOKEN_PROGRAM, keys: [{ pubkey: w, isWritable: true, isSigner: false }], data: Buffer.from([17]) }),
  ];
}

/** Accounts of `reinvest_lp` for anchor's accountsStrict. */
export function reinvestLpAccounts(caller, k, dexConfig) {
  const g = globalKeys();
  const d = poolKeys(dexConfig, k);
  return {
    caller,
    config: g.config,
    launch: k.launch,
    mint: k.mint,
    nativeMint: WSOL,
    solVault: k.solVault,
    migrator: g.migrator,
    reinvestWsol: pda([enc('reinvest_wsol'), k.launch.toBuffer()]),
    reinvestToken: pda([enc('reinvest_token'), k.launch.toBuffer()]),
    pool: d.pool,
    position: d.position,
    positionNftAccount: d.positionNftAccount,
    poolTokenVault: d.poolTokenVault,
    poolWsolVault: d.poolWsolVault,
    poolAuthority: DAMM_POOL_AUTHORITY,
    dexEventAuthority: d.eventAuthority,
    dexProgram: DAMM,
    tokenProgram: TOKEN_PROGRAM,
    systemProgram: SYSTEM,
  };
}

/** Pending "back to liquidity" SOL below this waits (herdcall_core::dex::REINVEST_MIN_LAMPORTS). */
export const REINVEST_MIN_LAMPORTS = 10_000_000;

const MIN_SQRT_PRICE = 4295048016n;

/**
 * Of a DAMM v2 pool account's data: √price (Q64.64), liquidity, base fee numerator (over 1e9) and the SOL held by the
 * liquidity (ceil(L·(√p − √min) / 2^128), herdcall_core::dex::amount_b).
 */
export function readPool(data) {
  const u128 = (at) => data.readBigUInt64LE(at) + (data.readBigUInt64LE(at + 8) << 64n);
  const liquidity = u128(360);
  const sqrtPrice = u128(456);
  const prod = liquidity * (sqrtPrice - MIN_SQRT_PRICE);
  const sol = (prod + (1n << 128n) - 1n) >> 128n;
  return { liquidity, sqrtPrice, feeNumerator: data.readBigUInt64LE(8), sol };
}

/** Lamports one reinvest can take from this pool: min(0.5%, fee / 2) of its SOL (herdcall_core::dex). */
export function reinvestCap(pool) {
  const feeBps = (pool.feeNumerator * 10_000n) / 1_000_000_000n;
  const capBps = feeBps / 2n < 50n ? feeBps / 2n : 50n;
  return (pool.sol * capBps) / 10_000n;
}
