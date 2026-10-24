#!/usr/bin/env bash
# Deploys the devnet build of HerdCall and runs the whole launch flow on devnet (scripts/launch-flow.mjs).
# Needs: network access to api.devnet.solana.com, a funded payer (about 8 SOL: ~6.3 for the program with room to grow,
# ~2 for the flow),
# and programs/keys/herdcall-devnet.json (the program id keypair, kept out of git).
#   PAYER=~/.config/solana/id.json DEX_CONFIG=<optional> scripts/devnet.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
url="${URL:-https://api.devnet.solana.com}"
payer="${PAYER:-$HOME/.config/solana/id.json}"
cd "$root"
if [ -z "${DEX_CONFIG:-}" ]; then
  DEX_CONFIG="$(node scripts/find-dex-config.mjs --url "$url" --public | head -1 | cut -d' ' -f1)"
  [ -n "$DEX_CONFIG" ] || { echo "no usable public DAMM v2 config on $url" >&2; exit 1; }
fi
echo "DAMM config: $DEX_CONFIG"
cargo build-sbf --manifest-path programs/herdcall/Cargo.toml --features devnet -- --target-dir programs/target/devnet
solana program deploy -u "$url" -k "$payer" --program-id programs/keys/herdcall-devnet.json --max-len 900000 \
  programs/target/devnet/deploy/herdcall.so
node scripts/launch-flow.mjs --url "$url" --keypair "$payer" --dex-config "$DEX_CONFIG" --params devnet
