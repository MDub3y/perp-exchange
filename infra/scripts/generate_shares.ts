import { secp256k1 } from '@noble/curves/secp256k1.js';
import { Client } from 'pg';
import { exec, ChildProcess } from 'child_process';
import * as crypto from 'crypto';

const q = BigInt('0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141');

const BATCH_SIZE = 10;
const APP_DB_URL = "postgres://exchange_admin:exchange_secure_password@127.0.0.1:5432/exchange_v1";

const NODE_CONFIGS = [
    { id: 1, port: 6431, url: 'postgres://mpc_operator_1:supersecurepasswordnode1@127.0.0.1:6431/postgres', context: 'kind-mpc-node-1' },
    { id: 2, port: 6432, url: 'postgres://mpc_operator_2:supersecurepasswordnode2@127.0.0.1:6432/postgres', context: 'kind-mpc-node-2' },
    { id: 3, port: 6433, url: 'postgres://mpc_operator_3:supersecurepasswordnode3@127.0.0.1:6433/postgres', context: 'kind-mpc-node-3' },
];

function randomScalar(): bigint {
    while (true) {
        const bytes = crypto.randomBytes(32);
        const num = BigInt('0x' + bytes.toString('hex'));
        if (num > 0n && num < q) return num;
    }
}

function startPortForward(context: string, localPort: number): Promise<ChildProcess> {
    return new Promise((resolve, reject) => {
        const process = exec(`kubectl --context ${context} port-forward statefulset/postgres -n mpc-crypto ${localPort}:5432`);

        setTimeout(() => {
            if (process.killed) {
                reject(new Error(`Failed to map port forward on port ${localPort}`));
            } else {
                resolve(process);
            }
        }, 2000);
    });
}

async function main() {
    console.log(`Starting FROST setup for ${BATCH_SIZE} pre-allocated accounts...`);

    const tunnels: ChildProcess[] = [];

    try {
        for (const node of NODE_CONFIGS) {
            console.log(`Opening tunnel for ${node.context} on port ${node.port}...`);
            const proc = await startPortForward(node.context, node.port);
            tunnels.push(proc);
        }

        const appClient = new Client({ connectionString: APP_DB_URL });
        await appClient.connect();

        const nodeClients = await Promise.all(
            NODE_CONFIGS.map(async (node) => {
                const client = new Client({ connectionString: node.url });
                await client.connect();
                return client;
            })
        );

        console.log("Connected to all target db clusters.");

        for (let i = 0; i < BATCH_SIZE; i++) {
            const a0 = randomScalar();
            const a1 = randomScalar();

            const a0Hex = a0.toString(16).padStart(64, '0');
            const a0Bytes = Buffer.from(a0Hex, 'hex');

            const masterPubKeyBytes = secp256k1.getPublicKey(a0Bytes, false);
            const masterPubKeyHex = Buffer.from(masterPubKeyBytes).toString('hex');

            const s1 = (a0 + a1 * 1n) % q;
            const s2 = (a0 + a1 * 2n) % q;
            const s3 = (a0 + a1 * 3n) % q;

            await appClient.query(
                'INSERT INTO mpc_accounts_pool (public_key, user_id, assigned_at) VALUES ($1, NULL, NULL) ON CONFLICT DO NOTHING',
                [masterPubKeyHex]
            );

            const shares = [s1, s2, s3];
            for (let nodeIdx = 0; nodeIdx < 3; nodeIdx++) {
                await nodeClients[nodeIdx].query(
                    'INSERT INTO mpc_shares (public_key, key_share, user_id, username, email, assigned_at) VALUES ($1, $2, NULL, NULL, NULL, NULL) ON CONFLICT DO NOTHING',
                    [masterPubKeyHex, shares[nodeIdx].toString(16)]
                );
            }
        }

        console.log("Seeded DB");

        await appClient.end();
        for (const client of nodeClients) {
            await client.end();
        }
    } catch (error) {
        console.error('<<<<<Cryptographic generation execution failed:', error);
    } finally {
        console.log('Tearing down background communication tunnels...');
        for (const proc of tunnels) {
            proc.kill('SIGINT');
        }
        process.exit(0);
    }
}

main();