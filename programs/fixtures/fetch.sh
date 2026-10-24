#!/usr/bin/env bash
# Downloads the third-party programs the LiteSVM tests load, from mainnet, and checks them against SHA256SUMS.
# A checksum mismatch means the program was upgraded on mainnet: review the change, then refresh SHA256SUMS.
set -euo pipefail
cd "$(dirname "$0")"
RPC="${RPC:-https://api.mainnet-beta.solana.com}"
solana program dump -u "$RPC" metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s mpl_token_metadata.so
solana program dump -u "$RPC" cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG damm_v2.so
sha256sum -c SHA256SUMS
