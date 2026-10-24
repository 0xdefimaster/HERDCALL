#!/usr/bin/env bash
# Starts a local validator with the HerdCall program (mainnet build), the real Metaplex and Meteora DAMM v2 programs
# and a private DAMM config whose pool creator is the HerdCall migrator: the same setup mainnet needs.
# DEVNET=1 uses the devnet build and a public config instead, as on devnet.
# The program is upgradeable with KEYPAIR (default ~/.config/solana/id.json) as its upgrade authority, as after a real
# deploy: only that key can initialize. Then, in another shell:
#   node scripts/launch-flow.mjs --url http://127.0.0.1:8899 --keypair <KEYPAIR> --dex-config AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW --params example
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
fx="$root/programs/fixtures"
ledger="${LEDGER:-$root/programs/target/test-ledger}"
authority="$(solana-keygen pubkey "${KEYPAIR:-$HOME/.config/solana/id.json}")"
# The program id: HERDCALL_PROGRAM_ID if set (the build reads it too), else the default in programs/herdcall/src/lib.rs.
program_id="${HERDCALL_PROGRAM_ID:-EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K}"
[ -f "$fx/damm_v2.so" ] || "$fx/fetch.sh"
if [ -n "${DEVNET:-}" ]; then
  cargo build-sbf --manifest-path "$root/programs/herdcall/Cargo.toml" --features devnet -- --target-dir "$root/programs/target/devnet"
  so="$root/programs/target/devnet/deploy/herdcall.so"
else
  cargo build-sbf --manifest-path "$root/programs/herdcall/Cargo.toml"
  so="$root/programs/target/deploy/herdcall.so"
fi
node --input-type=module -e "
import { readFileSync, writeFileSync } from 'node:fs';
import { globalKeys } from '$root/scripts/herdcall-client.mjs';
const d = readFileSync('$fx/damm_config_1pct.bin');
if (!process.env.DEVNET) globalKeys().migrator.toBuffer().copy(d, 40);
writeFileSync('$ledger.config.json', JSON.stringify({ pubkey: 'AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW', account: {
  lamports: 3173760, data: [d.toString('base64'), 'base64'], owner: 'cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG',
  executable: false, rentEpoch: 0, space: d.length } }));
"
exec solana-test-validator --reset --quiet --ledger "$ledger" \
  --upgradeable-program "$program_id" "$so" "$authority" \
  --bpf-program metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s "$fx/mpl_token_metadata.so" \
  --bpf-program cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG "$fx/damm_v2.so" \
  --account AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW "$ledger.config.json"
