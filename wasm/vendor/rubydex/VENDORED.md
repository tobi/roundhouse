# Vendored `rubydex` (wasm32 build support)

This is a temporary, in-repo copy of `rubydex` 0.2.5 carrying the 32-bit build
fix that is upstream-pending as **Shopify/rubydex#1059** (branch
`rmf-wasm32-mem-size-assertions`). `wasm/Cargo.toml` patches `rubydex` to this
path so the browser compiler (`roundhouse_wasm.wasm`) can build for
`wasm32-wasip1`.

## Why

Rubydex's `assert_mem_size!` checks struct sizes at compile time against the
64-bit layout. On `wasm32` pointers are 32 bits, so `Ancestors`, `Declaration`,
`Namespace` and others are smaller, and the published crate fails to compile
(E0080, E0308). #1059 gates both checks with
`#[cfg(target_pointer_width = "64")]`; native builds keep them unchanged.

Only the wasm crate uses this copy. The native `roundhouse` build depends on
the published `rubydex = "=0.2.5"`.

## Provenance (how to regenerate)

- Everything except this file: the published `rubydex` 0.2.5 crate from
  crates.io (upstream commit `14dc6259`, see `.cargo_vcs_info.json`), minus
  `.cargo-ok` and `Cargo.toml.orig`, which Cargo does not read.
- `src/compile_assertions.rs`: the two `#[cfg(target_pointer_width = "64")]`
  lines from #1059.

`diff -r ~/.cargo/registry/src/*/rubydex-0.2.5 wasm/vendor/rubydex` shows only
those two lines, the two omitted files, and this file.

## Removing this (when #1059 lands)

When a `rubydex` release with #1059 **publishes** to crates.io: delete this
directory and the `[patch.crates-io] rubydex` line in `wasm/Cargo.toml`, then
bump the `rubydex` pin in the root `Cargo.toml`. The `build-wasm` CI job keeps
working unchanged.
