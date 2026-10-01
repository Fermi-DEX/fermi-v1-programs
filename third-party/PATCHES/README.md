# Vendored crate patches

`third-party/` holds local copies of crates that the program build pulls in
through `[patch.crates-io]` (or, for `openbook-v2`, through a `path`
dependency) in the root `Cargo.toml`. The copies are what the deployed ELF was
built with. Each `*.diff` here is the full `diff -ruN` between the crates.io
release of that version (or, for openbook-v2, the upstream git commit) and the
vendored copy, so a verifier can see exactly what was changed.

| Vendored copy | Base | Patch |
|---|---|---|
| `ahash-0.7.8-local` | crates.io ahash 0.7.8 | `build.rs`: stop probing the host compiler for `specialize`/`stdsimd` cfgs (wrong compiler is probed under cargo-build-sbf) |
| `ahash-0.8.11-local` | crates.io ahash 0.8.11 | same `build.rs` change |
| `anchor-syn-0.28.0-local` | crates.io anchor-syn 0.28.0 | explicit `'__info` lifetimes on the generated `dispatch` fn (compiles on the pinned toolchain) |
| `anchor-attribute-program-0.28.0-local` | crates.io anchor-attribute-program 0.28.0 | depend on the local `anchor-syn` by path |
| `globset-0.4.16-local` | crates.io globset 0.4.16 | no source change (vendored to pin) |
| `num_enum-0.7.6-local` | crates.io num_enum 0.7.6 | drop `rust-version = "1.70.0"` |
| `num_enum_derive-0.7.6-local` | crates.io num_enum_derive 0.7.6 | drop `rust-version`; pin `proc-macro-crate = "=1.3.1"` |
| `proc-macro-crate-1.3.1-local` | crates.io proc-macro-crate 1.3.1 | no source change (vendored to pin) |
| `quote-1.0.45-local` | crates.io quote 1.0.45 | drop `rust-version = "1.71"` (host toolchain is 1.70) |
| `syn-2.0.117-local` | crates.io syn 2.0.117 | drop `rust-version = "1.71"` |
| `unicode-ident-1.0.24-local` | crates.io unicode-ident 1.0.24 | drop `rust-version = "1.71"` |
| `zmij-1.0.21-local` | crates.io zmij 1.0.21 | drop `rust-version = "1.71"` |
| `solana-program-1.16.14-local` | crates.io solana-program 1.16.14 | one `#[allow(invalid_reference_casting)]` so the crate compiles under platform-tools v1.41 |
| `openbook-v2-patched` | github.com/openbook-dex/openbook-v2 @ `f3e17421` (v0.2.10) | see `openbook-v2-patched.diff` (drops the switchboard oracle dependency, adapts to anchor 0.28 account-bumps API, misc. compile fixes). Only used when the optional `legacy-spot` feature is enabled; the deployed build does not enable it, but Cargo still resolves the crate. |

The `rust-version` removals exist because the deployed ELF was built with
Rust 1.70 (see `rust-toolchain.toml`), while those crate versions declare a
higher MSRV than they actually require.
