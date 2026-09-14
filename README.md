<p align="center">
  <img src="docs/banner.svg" alt="Tuitio" width="480">
</p>

# Tuitio · Smart Contracts

[![CI](https://github.com/tetedu/tuitio-contract/actions/workflows/ci.yml/badge.svg)](https://github.com/tetedu/tuitio-contract/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Network](https://img.shields.io/badge/network-Stellar%20testnet-7d00ff)](https://stellar.expert/explorer/testnet/contract/CD2INHSYNIQTVWM222CNSS7MN4MJSOBZOXVEWY3SOAK6MHICCZIEVQWV)

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
  owed payment by withholding a signature.
- **Cancellation** refunds every unsettled term, and is refused while a term
  is attested or disputed — the sponsor cannot pull funds out from under an
  open claim.

## Deployed on testnet

| Contract | Address |
|---|---|
| institution-registry | [`CD2INHSYNIQTVWM222CNSS7MN4MJSOBZOXVEWY3SOAK6MHICCZIEVQWV`](https://stellar.expert/explorer/testnet/contract/CD2INHSYNIQTVWM222CNSS7MN4MJSOBZOXVEWY3SOAK6MHICCZIEVQWV) |
| tuition-escrow | [`CDV7FA3QJPBR7LORHCZCZG6UCRL7DMXVMZYRDLMW7EXUNBMN7SEXINXW`](https://stellar.expert/explorer/testnet/contract/CDV7FA3QJPBR7LORHCZCZG6UCRL7DMXVMZYRDLMW7EXUNBMN7SEXINXW) |

Deployments are recorded in [`deployments/`](deployments/).

## Quick start

Requires Rust 1.96+ with the `wasm32v1-none` target and the Stellar CLI 27+.

```bash
cargo test                  # 52 tests across both contracts
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
| [adelekevictor12](https://github.com/adelekevictor12) | Maintainer | adelekevat@gmail.com |

## Contributing

Issues labeled `Stellar Wave` are part of the [Drips Wave](https://www.drips.network/wave/stellar)
program and carry point values. See [CONTRIBUTING.md](CONTRIBUTING.md). The
contracts are **unaudited** — see [SECURITY.md](SECURITY.md).

## License

Apache-2.0. See [LICENSE](LICENSE).
