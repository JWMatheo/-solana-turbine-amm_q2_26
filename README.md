# Solana AMM — Week 3 assignment

This repository contains a constant-product automated market maker built with
Anchor and tested with LiteSVM. The implementation covers the mandatory parts
of the Week 3 assignment:

- pool initialization;
- liquidity deposits and withdrawals with LP tokens;
- swaps in both directions with slippage protection;
- basis-point swap fees and dedicated treasury accounts;
- fee collection by the configured pool authority;
- integration tests for successful and rejected instructions.

## Architecture

Each pool is identified by `seed` and has a `Config` PDA:

| Account                    | Derivation / role                                                          |
| -------------------------- | -------------------------------------------------------------------------- |
| `config`                   | `config` + `seed`; stores the mints, fee, authority and treasury addresses |
| `mint_lp`                  | `lp` + `config`; mint authority is the `config` PDA                        |
| `vault_x`, `vault_y`       | Associated token accounts owned by `config`; pool liquidity                |
| `treasury_x`, `treasury_y` | Token-account PDAs owned by `config`; protocol fees in the input token     |

Fees are expressed in basis points (`30` = `0.30%`) and are limited to
`0..=10_000`. A swap first calculates the fee-adjusted input, deposits the net
amount in the corresponding pool vault, and sends the fee to the treasury for
that input mint. `collect_fees` transfers both treasury balances to the
configured authority's token accounts.

## Prerequisites

- Rust toolchain selected by `rust-toolchain.toml`;
- Solana CLI;
- Anchor CLI;
- Yarn, as specified in `Anchor.toml`.

The repository pins a compatible stable toolchain: Rust `1.98.1` with
`rustfmt`, `clippy` and `rust-analyzer`, Anchor CLI/framework `1.2.0`, Solana
CLI `4.2.2`, and LiteSVM `0.16.0` for the Rust integration tests.

## Build and test

Run the complete suite with:

```bash
anchor test
```

The suite uses LiteSVM, so no external validator is required by the Rust test
cases. A direct `cargo test` needs the deployable program binary first:

```bash
anchor build
cargo test
```

The expected result is three passing unit tests and ten passing integration
tests. The integration tests cover initialization, deposits, withdrawal, both
swap directions, fee collection, invalid fees, zero deposits and slippage
rejection.

## Relevant code

- Program entrypoints: `programs/amm-video/src/lib.rs`
- Pool state: `programs/amm-video/src/state.rs`
- Initialization and treasury creation: `programs/amm-video/src/instructions/initialize.rs`
- Swap fee routing: `programs/amm-video/src/instructions/swap.rs`
- Fee collection: `programs/amm-video/src/instructions/collect_fees.rs`
- Integration tests: `programs/amm-video/tests/tests.rs`

## Scope and limitations

This is an educational AMM using the legacy SPL Token program and the
`constant-product-curve` library. It is not presented as production-ready
financial software. The optional from-scratch CPMM and downtime-mitigation
extension are outside the scope of this submission.
