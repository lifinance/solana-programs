#!/usr/bin/env bash
set -euo pipefail

SO_FILE="$1"
SIM_DIR="$(cd "$(dirname "$0")" && pwd)"

# Fully isolated config — never touches ~/.config/solana
SOLANA_CONFIG="$SIM_DIR/sim-config.yml"
KEYPAIR="$SIM_DIR/sim-keypair.json"
LEDGER="$SIM_DIR/sim-ledger"

SOLANA="solana --config $SOLANA_CONFIG"

cleanup() {
  echo ""
  echo "--- Cleaning up ---"
  kill "$VALIDATOR_PID" 2>/dev/null || true
  rm -f "$KEYPAIR" "$SOLANA_CONFIG"
  rm -rf "$LEDGER"
  echo "Done. No keypairs or configs left behind."
}
trap cleanup EXIT

# 1. Generate isolated keypair
solana-keygen new --no-bip39-passphrase --silent --outfile "$KEYPAIR"
ADDR=$(solana-keygen pubkey "$KEYPAIR")
echo "Isolated keypair: $ADDR"

# 2. Write isolated config pointing to localhost
cat > "$SOLANA_CONFIG" <<EOF
json_rpc_url: "http://127.0.0.1:8899"
websocket_url: ""
keypair_path: "$KEYPAIR"
address_labels: {}
commitment: confirmed
EOF

# 3. Start validator using isolated ledger dir
solana-test-validator \
  --ledger "$LEDGER" \
  --reset \
  --quiet &
VALIDATOR_PID=$!
echo "Validator PID: $VALIDATOR_PID"

# Wait for it to be ready
for i in $(seq 1 20); do
  if $SOLANA cluster-version 2>/dev/null | grep -q "solana-core"; then
    echo "Validator ready after ${i}s"
    break
  fi
  sleep 1
done

# 4. Airdrop enough SOL (no real funds involved)
$SOLANA airdrop 5 "$ADDR"
sleep 1

BEFORE=$($SOLANA balance "$ADDR" --lamports | awk '{print $1}')
echo "Balance before deploy: $BEFORE lamports"

# 5. Deploy the program
echo ""
echo "--- Deploying $SO_FILE ---"
$SOLANA program deploy "$SO_FILE" --keypair "$KEYPAIR"

# 6. Measure cost
AFTER=$($SOLANA balance "$ADDR" --lamports | awk '{print $1}')
echo ""
echo "Balance after deploy:  $AFTER lamports"

COST=$((BEFORE - AFTER))
SOL_COST=$(echo "scale=9; $COST / 1000000000" | bc)

echo ""
echo "========================================"
echo "  DEPLOY COST SIMULATION RESULT"
echo "========================================"
echo "  Lamports spent : $COST"
echo "  SOL spent      : $SOL_COST SOL"
echo "  (rent-exempt deposit is recoverable)"
echo "========================================"
