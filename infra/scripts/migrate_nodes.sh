#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"

echo "Starting cross-cluster MPC database migration..."

declare -A CREDENTIALS=(
    ["1"]="mpc_operator_1:supersecurepasswordnode1@mpc_shares_1"
    ["2"]="mpc_operator_2:supersecurepasswordnode2@mpc_shares_2"
    ["3"]="mpc_operator_3:supersecurepasswordnode3@mpc_shares_3"
)

for i in {1..3}; do
    CLUSTER_NAME="mpc-node-$i"
    CONTEXT_NAME="kind-$CLUSTER_NAME"
    LOCAL_PORT=$((6430 + i)) # Node 1 -> 6431, Node 2 -> 6432, Node 3 -> 6433
    
    echo "-------------------------------------------------"
    echo "Targeting Cluster: $CLUSTER_NAME (Context: $CONTEXT_NAME)"
    
    kubectl config use-context "$CONTEXT_NAME" > /dev/null
    
    echo "Opening temporary tunnel to Postgres on local port $LOCAL_PORT..."
    kubectl port-forward statefulset/postgres -n mpc-crypto "$LOCAL_PORT":5432 > /dev/null 2>&1 &
    FORWARD_PID=$!
    
    sleep 2
    
    export DATABASE_URL="postgres://${CREDENTIALS[$i]}@127.0.0.1:$LOCAL_PORT"
    
    echo "Pushing Prisma schema to database..."
    npx prisma db push --schema=./prisma/schema.prisma
    
    echo "Closing tunnel (PID: $FORWARD_PID)..."
    kill "$FORWARD_PID"
    wait "$FORWARD_PID" 2>/dev/null || true
    
    echo "Node $i schema updated successfully."
done

echo "-------------------------------------------------"
echo "All isolated MPC nodes successfully synchronized with the latest schema!"