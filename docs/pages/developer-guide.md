# Developer guide

Three repositories make up Tuitio:

| Repo | Stack | Role |
|---|---|---|
| [tuitio-contract](https://github.com/tetedu/tuitio-contract) | Rust / Soroban SDK 27 | the two contracts |
| [tuitio-backend](https://github.com/tetedu/tuitio-backend) | Go 1.25 / Postgres | event indexer + REST API |
| [tuitio-frontend](https://github.com/tetedu/tuitio-frontend) | Next.js 16 / TypeScript | web app + wallet actions |

## Local setup

Contracts (Rust 1.96+, `wasm32v1-none`, Stellar CLI 27+):

```bash
git clone https://github.com/tetedu/tuitio-contract
cd tuitio-contract
cargo test               # 52 tests
stellar contract build
```

Backend + frontend, zero provisioning (embedded Postgres, testnet indexer,
API, web app):

```bash
git clone https://github.com/tetedu/tuitio-backend
cd tuitio-backend && go run ./cmd/devstack   # API on :8080

git clone https://github.com/tetedu/tuitio-frontend
cd tuitio-frontend
pnpm install
cp .env.example .env.local                   # NEXT_PUBLIC_API_URL=http://localhost:8080
pnpm dev                                     # web app on :3000
```

## Environment variables

**tuitio-backend**

| Variable | Meaning |
|---|---|
| `DATABASE_URL` | Postgres connection string |
| `SOROBAN_RPC_URL` | RPC endpoint (default: public testnet) |
| `REGISTRY_CONTRACT` | institution-registry contract ID |
| `ESCROW_CONTRACT` | tuition-escrow contract ID |
| `START_LEDGER` | 0 = start at chain tip; lower = backfill history |
| `POLL_SECONDS` | indexer poll interval (default 10) |
| `PORT` | HTTP port (default 8080) |

**tuitio-frontend** — `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_RPC_URL`,
`NEXT_PUBLIC_NETWORK_PASSPHRASE`, `NEXT_PUBLIC_REGISTRY_CONTRACT`,
`NEXT_PUBLIC_ESCROW_CONTRACT`, `NEXT_PUBLIC_TOKEN_CONTRACT`,
`NEXT_PUBLIC_TOKEN_SYMBOL`, `NEXT_PUBLIC_TOKEN_DECIMALS` (see
[.env.example](https://github.com/tetedu/tuitio-frontend/blob/main/.env.example)).

## REST API

Base: `NEXT_PUBLIC_API_URL` (e.g. `http://localhost:8080`).

```
GET /healthz
GET /api/institutions
GET /api/institutions/{address}
GET /api/grants?sponsor=&institution=&status=
GET /api/grants/{id}
GET /api/grants/{id}/terms
GET /api/activity?limit=
GET /api/stats
```

Example grant response:

```json
{
  "grant_id": 1,
  "sponsor": "GBDC3OQELLUAAZURNZ5ROOP34LQRBRTV3ZQZK7EUZTM2DYVALBS6P3JI",
  "beneficiary": "GASR6NC63LZVXH6UND2HAUVGIJPXYC7ZTTG453ACJDXD3CXU37GYTMA4",
  "institution": "GBSHG7BPNWLUQFM26QXI3ZDCOFPG5SSG4THHQRCAXQSPLTDBT7DEQ7XO",
  "token": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
  "term_amount": 500000000,
  "terms_total": 2,
  "next_term": 1,
  "status": "active",
  "locked_amount": 500000000
}
```

`term_amount` and `locked_amount` are integers in the token's smallest unit
(XLM: 7 decimals, so 500000000 = 50 XLM).

## Interacting with the contracts directly

With the Stellar CLI (all commands need `--source-account` and
`--network testnet` in CLI 27):

```bash
# Read a grant
stellar contract invoke --id CDPGTHO2O7LTURTJF7L7ZZZTZCXPNABAPGVRDMBYIJ737R6O3DOPWJ34 \
  --source-account my-key --network testnet -- get_grant --grant_id 0

# Attest a term (institution wallet)
stellar contract invoke --id CDPGTHO2O7LTURTJF7L7ZZZTZCXPNABAPGVRDMBYIJ737R6O3DOPWJ34 \
  --source-account school-key --network testnet --send=yes \
  -- attest_term --grant_id 0 --term_index 0
```

From TypeScript, the frontend uses the stellar-sdk contract `Client` with
Freighter signing — see
[lib/contracts.ts](https://github.com/tetedu/tuitio-frontend/blob/main/lib/contracts.ts).

## Deploying the contracts

```bash
SOURCE=my-key ./scripts/deploy_testnet.sh
```

Deploys the registry first (the escrow stores its address at initialization),
then the escrow, then initializes both, printing the contract IDs.

## Tests

- Contract: `cargo test` — 52 unit tests.
- Backend: `go test ./...` — unit + embedded-Postgres integration; plus
  `TestLiveIngestion` (env-guarded) which runs the entire pipeline against
  testnet.
- Frontend: `pnpm lint && pnpm build`.
