# FLP vault

This directory contains the deployed FLP vault and its build dependencies.
The build produces only `flp_vault.so`.
The core source here is a library dependency. The root workspace builds the separate Fermi core program.

| Field | Value |
|---|---|
| Cluster | Solana mainnet-beta |
| Program | `FLPRrDPN5tJpLx6wY6FUiGFN5Kws8gHUCYMCcs6Goa9Q` |
| ProgramData | `Fekec2bVaipgZ6XsKEpSqFobSnjNEUgwfi4qxGNsq4Px` |
| Deployment slot | `461700429` |
| ELF bytes | `1776216` |
| ELF SHA-256 | `4cd0f7787be53a52195ff21bc34797f3eb40de1810d17443d883f1533f59f36e` |
| Solana executable hash | `5e03fd650b2d724636460edfa8f46d2b1435a4530f9999855d5573504b12c263` |
| Build features | `custom-heap,no-idl` |

## Build

Install Docker and `solana-verify` version `0.5.2`.
From the repository root, run:

```sh
bash flp/build.sh
```

The script uses the pinned Solana verification image and checks the complete ELF hash.
Cargo uses the lockfile in `programs/flp-vault/build`.
The nested directory layout preserves the source paths embedded in the deployed binary.
Blank source lines preserve compiler locations after comment removal. Do not reformat this snapshot before verification.

## Verify

Set `COMMIT` to the published commit that contains this snapshot.
Set `RPC_URL` to a mainnet RPC endpoint.
Run:

```sh
solana-verify verify-from-repo \
  --url "$RPC_URL" \
  --program-id FLPRrDPN5tJpLx6wY6FUiGFN5Kws8gHUCYMCcs6Goa9Q \
  --commit-hash "$COMMIT" \
  --mount-path flp/programs \
  --workspace-path flp/programs/flp-vault/build \
  --library-name flp_vault \
  --base-image solanafoundation/solana-verifiable-build@sha256:e9cfc73adb9d966d9aed5a10a4943726f42002fe25ac9506e3c32e4580e7d743 \
  https://github.com/Fermi-DEX/fermi-v1-programs
```

A local hash match does not create the explorer verification badge.
The upgrade authority must publish the verification record before OtterSec can verify it remotely.
After that transaction, submit the remote job:

```sh
solana-verify remote submit-job \
  --program-id FLPRrDPN5tJpLx6wY6FUiGFN5Kws8gHUCYMCcs6Goa9Q \
  --uploader 9o5DaZRiyS4cvaFy2jaPxPHKeKGfNt1PeUGgpPPWi3jw
```

See the [Solana verification guide](https://solana.com/docs/programs/verified-builds) for the explorer workflow.

## Licenses

Required license notices remain in `programs/LICENSE` and the dependency directories.
The core instruction library uses GPL-3.0. The hash dependencies retain their upstream licenses.
