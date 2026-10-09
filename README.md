<p align="center">
  <img src="docs/banner.svg" alt="Tuitio" width="480">
</p>

# Tuitio · Smart Contracts

[![CI](https://github.com/tetedu/tuitio-contract/actions/workflows/ci.yml/badge.svg)](https://github.com/tetedu/tuitio-contract/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Network](https://img.shields.io/badge/network-Stellar%20testnet-7d00ff)](https://stellar.expert/explorer/testnet/contract/CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO)

**Live demo:** <https://tuitio-frontend.vercel.app> (Stellar testnet, Freighter wallet)

**Soroban contracts that hold a student's tuition in escrow and release it to a verified institution, one academic term at a time.**

The problem: education money sent across borders leaks. A sponsor pays for a
term of school; the money arrives as general household cash and gets spent on
something more urgent. Tuitio binds the funds to the institution itself — the
student is recorded for attribution but can never touch the principal, and the
only address that can ever be paid is the school's registered payout address.

## How it works

Two contracts, one dependency:

1. **`institution-registry`** — answers one question: *is this address a
   legitimate tuition payee?* Registration is permissionless (records start
   `Pending`); only the registry admin promotes to `Verified`. Suspension
   blocks new grants and makes `payout_of` trap, so a suspended school cannot
   be paid.
2. **`tuition-escrow`** — holds funds and settles terms. Creating a grant
   transfers the **full commitment** (`term_amount × terms_total`) into the
   contract. The lifecycle per term:

```
Pending ──attest──▶ Attested ──(dispute window closes)──▶ Released
                        │                                    ▲
                     dispute                              resolve
                        ▼                                    │
                     Disputed ──────────── admin ─────────────┘
```

- **Attesting moves no money.** It starts a dispute window (7 days on
  testnet). This is deliberate: the institution both attests and gets paid,
  which is a real conflict of interest — the window plus sponsor dispute plus
  registry suspension are what constrain it.
- **Release is permissionless** once the window closes, so nobody can stall an
  owed payment by withholding a signature. `sweep` settles a batch of due
  grants in one transaction; entries that are not due — or whose institution
  has since been suspended — are skipped rather than failing the batch.
- **Cancellation** refunds every unsettled term, and is refused while a term
  is attested or disputed — the sponsor cannot pull funds out from under an
  open claim.

## Deployed on testnet

| Contract | Address |
|---|---|
| institution-registry | `CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO` |
| tuition-escrow | `CDPGTHO2O7LTURTJF7L7ZZZTZCXPNABAPGVRDMBYIJ737R6O3DOPWJ34` |

Verify them on-chain:

- Raw indexer data (JSON, no JavaScript required) —
  [registry](https://api.stellar.expert/explorer/testnet/contract/CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO)
  ·
  [escrow](https://api.stellar.expert/explorer/testnet/contract/CDPGTHO2O7LTURTJF7L7ZZZTZCXPNABAPGVRDMBYIJ737R6O3DOPWJ34)
- Block explorers —
  [StellarChain registry](https://testnet.stellarchain.io/contracts/CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO)
  ·
  [StellarChain escrow](https://testnet.stellarchain.io/contracts/CDPGTHO2O7LTURTJF7L7ZZZTZCXPNABAPGVRDMBYIJ737R6O3DOPWJ34)
  ·
  [StellarExpert registry](https://stellar.expert/explorer/testnet/contract/CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO)
- Or query them directly:

```bash
stellar contract invoke --id CCTZVSPMIYEA4X2Z6UP5IPIO2HBIZWHLV37OUDBV32YUCP64JNIPBCPO \
  --source-account <your-key> --network testnet -- count
```

Deployments are recorded in [`deployments/`](deployments/).

## Quick start

Requires Rust 1.96+ with the `wasm32v1-none` target and the Stellar CLI 27+.

```bash
cargo test                  # unit + property tests, both contracts
stellar contract build      # wasm artifacts into target/
SOURCE=my-key ./scripts/deploy_testnet.sh   # deploy + initialize, in order
```

The registry deploys **before** the escrow: the escrow stores the registry
address at initialization and cross-calls it on every grant creation.

## Repository layout

```
contracts/institution-registry/   verification of tuition payees
contracts/tuition-escrow/         grant lifecycle and fund custody
scripts/deploy_testnet.sh         ordered deploy + initialize
deployments/testnet.json          deployed contract addresses
```

## Related repositories

- [`tuitio-backend`](https://github.com/tetedu/tuitio-backend) — Go indexer and REST API serving the indexed read model
- [`tuitio-frontend`](https://github.com/tetedu/tuitio-frontend) — Next.js web app with Freighter wallet actions

## Maintainers

| Name | Role | Contact |
|---|---|---|
| [temieehade-coder](https://github.com/temieehade-coder) | Maintainer | temieehade@gmail.com |

## Contributing

Issues labeled `Stellar Wave` are part of the [Drips Wave](https://www.drips.network/wave/stellar)
program and carry point values. See [CONTRIBUTING.md](CONTRIBUTING.md). The
contracts are **unaudited** — see [SECURITY.md](SECURITY.md).

## License

Apache-2.0. See [LICENSE](LICENSE).
