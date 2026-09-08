#!/usr/bin/env bash
# Deploys the Tuitio contracts to Stellar testnet in dependency order.
#
# The registry must exist before the escrow, because the escrow stores the
# registry address at initialization and consults it on every grant.
#
# Usage: SOURCE=my-key ./scripts/deploy_testnet.sh
set -euo pipefail

SOURCE="${SOURCE:-tuitio-deployer}"
NETWORK="${NETWORK:-testnet}"
# Seconds a sponsor has to dispute an attestation. 7 days.
DISPUTE_WINDOW="${DISPUTE_WINDOW:-604800}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
OUT="deployments/${NETWORK}.json"
WASM_DIR="target/wasm32v1-none/release"

echo "==> Building contracts"
stellar contract build

ADMIN="$(stellar keys address "$SOURCE")"
echo "==> Deployer / admin: $ADMIN"

echo "==> [1/2] Deploying institution-registry"
REGISTRY_ID="$(stellar contract deploy \
  --wasm "$WASM_DIR/institution_registry.wasm" \
  --source "$SOURCE" --network "$NETWORK" 2>/dev/null | tail -1)"
echo "    registry: $REGISTRY_ID"

echo "==> Initializing institution-registry"
stellar contract invoke --id "$REGISTRY_ID" --source "$SOURCE" --network "$NETWORK" \
  -- initialize --admin "$ADMIN" >/dev/null

echo "==> [2/2] Deploying tuition-escrow"
ESCROW_ID="$(stellar contract deploy \
  --wasm "$WASM_DIR/tuition_escrow.wasm" \
  --source "$SOURCE" --network "$NETWORK" 2>/dev/null | tail -1)"
echo "    escrow: $ESCROW_ID"

echo "==> Initializing tuition-escrow"
stellar contract invoke --id "$ESCROW_ID" --source "$SOURCE" --network "$NETWORK" \
  -- initialize \
     --admin "$ADMIN" \
     --registry "$REGISTRY_ID" \
     --dispute_window "$DISPUTE_WINDOW" >/dev/null

cat > "$OUT" <<JSON
{
  "network": "$NETWORK",
  "admin": "$ADMIN",
  "dispute_window_seconds": $DISPUTE_WINDOW,
  "contracts": {
    "institution_registry": "$REGISTRY_ID",
    "tuition_escrow": "$ESCROW_ID"
  },
  "deployed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
JSON

cat <<SUMMARY

================================================================
 Tuitio deployed to $NETWORK
================================================================

NEXT_PUBLIC_STELLAR_NETWORK=$NETWORK
NEXT_PUBLIC_REGISTRY_CONTRACT_ID=$REGISTRY_ID
NEXT_PUBLIC_ESCROW_CONTRACT_ID=$ESCROW_ID
NEXT_PUBLIC_SOROBAN_RPC_URL=https://soroban-testnet.stellar.org

Explorer:
  https://stellar.expert/explorer/$NETWORK/contract/$REGISTRY_ID
  https://stellar.expert/explorer/$NETWORK/contract/$ESCROW_ID

Written to $OUT
================================================================
SUMMARY
