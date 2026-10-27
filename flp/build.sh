#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
image='solanafoundation/solana-verifiable-build@sha256:e9cfc73adb9d966d9aed5a10a4943726f42002fe25ac9506e3c32e4580e7d743'
solana-verify --version | grep -Eq '(^| )0\.5\.2($| )'
solana-verify build "$root/.." \
  --workspace-path "$root/../flp-vault/build" \
  --library-name flp_vault --base-image "$image"
printf '%s  %s\n' '4cd0f7787be53a52195ff21bc34797f3eb40de1810d17443d883f1533f59f36e' \
  "$root/../flp-vault/build/target/deploy/flp_vault.so" | sha256sum --check
