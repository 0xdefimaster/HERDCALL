// Lists DAMM v2 configs a HerdCall breakout accepts (static, fees in SOL, full price range, constant fee <= 2%; see
// programs/herdcall/src/damm.rs). With --public, configs anyone can use (devnet build only); otherwise private configs
// whose pool creator is the HerdCall migrator.
//   node scripts/find-dex-config.mjs --url https://api.devnet.solana.com --public
import anchor from '@anchor-lang/core';
import { DAMM, globalKeys } from './herdcall-client.mjs';

const { web3 } = anchor;
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : fallback;
};
const url = arg('url', 'https://api.devnet.solana.com');
const creator = process.argv.includes('--public') ? web3.PublicKey.default : globalKeys().migrator;
const MIN = 4295048016n;
const MAX = 79226673521066979257578248091n;
const u128 = (b, at) => b.readBigUInt64LE(at) + (b.readBigUInt64LE(at + 8) << 64n);

const connection = new web3.Connection(url, 'confirmed');
const accounts = await connection.getProgramAccounts(DAMM, {
  filters: [{ dataSize: 328 }, { memcmp: { offset: 40, bytes: creator.toBase58() } }],
});
const ok = [];
for (const { pubkey, account } of accounts) {
  const d = account.data.subarray(8);
  const fee = d.readBigUInt64LE(64);
  const periods = d.readUInt16LE(78);
  if (fee > 20_000_000n || periods !== 0 || d[72] !== 0 || d[193] !== 1 || d[194] !== 0) continue;
  if (u128(d, 208) !== MIN || u128(d, 224) !== MAX) continue;
  ok.push({ config: pubkey.toBase58(), feePercent: Number(fee) / 1e7 });
}
ok.sort((a, b) => a.feePercent - b.feePercent);
console.error(`${accounts.length} configs with pool creator ${creator.toBase58()}, ${ok.length} usable`);
for (const c of ok) console.log(`${c.config} ${c.feePercent}%`);
