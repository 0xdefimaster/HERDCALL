#!/usr/bin/env bash
# Downloads the third-party programs the LiteSVM tests load, from mainnet, and checks them against SHA256SUMS.
# A checksum mismatch means the program was upgraded on mainnet: review the change, then refresh SHA256SUMS.
set -euo pipefail
cd "$(dirname "$0")"
RPC="${RPC:-https://api.mainnet-beta.solana.com}"
solana program dump -u "$RPC" metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s mpl_token_metadata.so
solana program dump -u "$RPC" cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG damm_v2.so
# A public static DAMM v2 config (1% constant fee, fees in SOL, full price range). Tests turn it into a private config
# for the HerdCall migrator PDA, which is what mainnet needs (document/CURVE_SPEC.md §5.4).
solana account -u "$RPC" AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW --output-file damm_config_1pct.bin >/dev/null
sha256sum -c SHA256SUMS
