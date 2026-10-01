# fermi-v1-programs

Source of the Fermi v1 on-chain program as deployed on Solana mainnet, published
so that anyone can rebuild the program and compare it byte-for-byte with the
deployed binary.

This repository contains **only** the on-chain program and what is needed to
build it. Off-chain components (relayer, harness, clients, tooling) live in
the main `fermi-v1` repository and are not part of the verified artifact.

## Deployed program

| | |
|---|---|
| Program id | `FRMiX94PWHbndvySq8xyRx185GvPeggUeKaLmZgVdd4` |
| Cluster | mainnet-beta |
| ProgramData account | `EMGzJcQNA8Ua7wMtkMmBoi1WBMgvUjQREkGdHxjeP2S7` |
| Deployed in slot | 451,279,020 (2026-09-28) |
| Source commit (fermi-v1) | `ea0a609c0283b1f9c475d269667ce5ccd049b216` |
| ELF SHA-256 | `29c6167e1e0eb4577215cf5a85a9646ed36b97c479ace0a71631b7fe71358aa9` |
| ELF size | 3,777,264 bytes |
| Build features | `enable-gpl` |

The program crate is `programs/mango-v4` (crate name `mango-v4`, library
`mango_v4`; the program is a fork of Mango v4). `mango_v4.json` is the Anchor
IDL matching this build.

## Toolchain

The deployed ELF was produced with:

| Tool | Version |
|---|---|
| `solana-cli` / `cargo build-sbf` | 1.18.26 |
| platform-tools | v1.41 |
| Rust (host, from `rust-toolchain.toml`) | 1.70 |

Dependency versions are pinned by `Cargo.lock`. Several crates are vendored
under `third-party/` and wired in through `[patch.crates-io]` (and one `path`
dependency) in the root `Cargo.toml`; they must be used as-is for a
byte-identical build. Each vendored crate's difference from its crates.io or
upstream release is recorded in `third-party/PATCHES/` (see the README there).

## Build

```bash
# Solana 1.18.26 toolchain
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo build-sbf --version   # expect solana-cargo-build-sbf 1.18.26, platform-tools v1.41

git clone https://github.com/Fermi-DEX/fermi-v1-programs
cd fermi-v1-programs
cargo build-sbf --manifest-path programs/mango-v4/Cargo.toml --features enable-gpl
sha256sum target/deploy/mango_v4.so
# 29c6167e1e0eb4577215cf5a85a9646ed36b97c479ace0a71631b7fe71358aa9
```

`--features enable-gpl` is required: parts of the program are GPL-licensed and
the crate refuses to compile without opting in (see `LICENSE`).

## Verify against mainnet

The upgradeable loader stores the ELF at the start of the ProgramData account,
zero-padded to the account's size. Compare the first 3,777,264 bytes of the
dump with the ELF you built:

```bash
solana program dump FRMiX94PWHbndvySq8xyRx185GvPeggUeKaLmZgVdd4 onchain.so \
  --url https://api.mainnet-beta.solana.com
head -c 3777264 onchain.so | sha256sum
# 29c6167e1e0eb4577215cf5a85a9646ed36b97c479ace0a71631b7fe71358aa9
tail -c +3777265 onchain.so | tr -d '\0' | wc -c
# 0  (everything after the ELF is padding)
```

Alternatively, with [`solana-verify`](https://github.com/Ellipsis-Labs/solana-verifiable-build):

```bash
solana-verify verify-from-repo -um \
  --program-id FRMiX94PWHbndvySq8xyRx185GvPeggUeKaLmZgVdd4 \
  --library-name mango_v4 \
  -- --features enable-gpl \
  https://github.com/Fermi-DEX/fermi-v1-programs
```

(`solana-verify` builds inside a Docker image; use one that ships Solana
1.18.26 / platform-tools v1.41 so the output is byte-identical.)

## Layout

```
programs/mango-v4/   the program crate (source, unit/integration tests, fixtures)
third-party/         vendored crates referenced from [patch.crates-io]
Cargo.toml           workspace: the program only, same pins and patches as fermi-v1
Cargo.lock           dependency lock from the release commit
rust-toolchain.toml  host Rust toolchain pin
mango_v4.json        Anchor IDL
LICENSE              licensing (GPL-3.0 for programs/mango-v4/src/instructions, see file)
```

## Reproduction record

Rebuilt from this tree on 2026-10-01 with the toolchain above
(`cargo build-sbf --manifest-path programs/mango-v4/Cargo.toml --features enable-gpl`):

| | SHA-256 |
|---|---|
| `target/deploy/mango_v4.so` (this repo) | `29c6167e1e0eb4577215cf5a85a9646ed36b97c479ace0a71631b7fe71358aa9` |
| mainnet ProgramData, first 3,777,264 bytes (dumped 2026-10-01 04:13 UTC) | `29c6167e1e0eb4577215cf5a85a9646ed36b97c479ace0a71631b7fe71358aa9` |
| `Cargo.lock` in this repo (program-only workspace) | `09d5d8f2b52feaa46b2481b76168cf99792e7a1a6bd30ea680b1594412e2bfba` |
| `Cargo.lock` of fermi-v1 at the release commit (full workspace) | `5c5d33e6a635c958a6755ff6d96ea1944441bba169bddc618c4f0976fa5111cd` |

The lockfile here is the release lockfile with the entries that only the
removed off-chain crates used pruned by Cargo (823 → 545 packages). Every
remaining package has the same version and checksum as in the release lockfile,
with one exception that does not enter the ELF: `jsonrpc-core 18.0.0` (a
dev-dependency reached through `solana-program-test`) now comes from crates.io,
whereas the full workspace took it from the `ckamm/jsonrpc` git fork that its
off-chain crates patch in. `cargo build-sbf` compiles the library without
dev-dependencies, which is why the ELF hash is unchanged.

## Provenance

- `programs/mango-v4`, `Cargo.lock`, `rust-toolchain.toml`, `LICENSE` and
  `mango_v4.json` were exported with `git archive` from fermi-v1 commit
  `ea0a609c0283b1f9c475d269667ce5ccd049b216`.
- `third-party/` is git-ignored in fermi-v1 (only the two `ahash` copies are
  tracked there), so the release commit alone does not reproduce the ELF. The
  vendored crates were copied from the build machine's working tree as used for
  the deployment (no file newer than the deployed ELF), with VCS and build
  directories removed; `third-party/PATCHES/` documents each one against its
  upstream source.
- The root `Cargo.toml` keeps the release workspace's `[workspace.dependencies]`,
  `[profile.release]` and `[patch.crates-io]` tables verbatim (the vendored
  openbook-v2 crate inherits its dependency versions from them) and reduces
  `members` to the program. `Anchor.toml` is reduced to the program id.
- No program source, vendored crate or lockfile entry was edited by hand. The
  repository carries no history by design: the single commit is the
  verification snapshot.
