#!/usr/bin/env bash
# Builds the HerdCall program for a deploy under a given program id and prints its SHA256, pinned so that the same
# commit gives the same bytes on another machine: the platform-tools compiler (--tools-version), the SBPF version
# (--arch), the dependency versions (Cargo.lock, --locked) and the release profile in programs/Cargo.toml. The binary
# holds no paths of the machine that builds it (only relative ones and the platform-tools' own).
#   HERDCALL_PROGRAM_ID=<program address> scripts/build-release.sh
set -euo pipefail
: "${HERDCALL_PROGRAM_ID:?set HERDCALL_PROGRAM_ID to the program address}"
root="$(cd "$(dirname "$0")/.." && pwd)"
tools=v1.57
arch=v3
echo "commit       $(git -C "$root" rev-parse HEAD)$(git -C "$root" diff --quiet HEAD -- programs || echo ' (programs/ has local changes)')"
echo "solana       $(solana --version)"
echo "build-sbf    $(cargo build-sbf --version | head -1)"
echo "platform     $tools, arch $arch"
echo "program id   $HERDCALL_PROGRAM_ID"
cargo build-sbf --manifest-path "$root/programs/herdcall/Cargo.toml" --tools-version "$tools" --arch "$arch" -- --locked >/dev/null
so="$root/programs/target/deploy/herdcall.so"
echo "size         $(wc -c < "$so") bytes"
echo "sha256       $(sha256sum "$so" | cut -d' ' -f1)"
