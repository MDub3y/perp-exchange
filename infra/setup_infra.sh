#!/usr/bin/env bash

set -euo pipefail

echo "Starting isolated MPC infrastructure deployment..."

for i in {1..3}; do
    CLUSTER_NAME="mpc-node-$i"
    CONTEXT_NAME="kind-$CLUSTER_NAME"

    echo "-------------------------------------------------"
    echo "Creating cluster: $CLUSTER_NAME..."

    if kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
        echo "Cluster $CLUSTER_NAME already exists. Skipping creation."
    else 
        kind create cluster --config "infra/clusters/node$i.yaml"
    fi
    
    echo "Switching kubernetes context to $CONTEXT_NAME..."
    kubectl config use-context "$CONTEXT_NAME"

    echo "Applying isolated PostgreSQL StatefulSet..."
    kubectl apply -f "infra/mpc-nodes/node$i-db.yaml"

    echo "Waiting for PostgreSQL pod to be ready in $CLUSTER_NAME..."
    kubectl rollout status statefulset/postgres -n mpc-crypto --timeout=90s

    echo "Node $i successfully initialized."
done

echo "-------------------------------------------------"
echo "All 3 isolated MPC database backends are fully commissioned!"