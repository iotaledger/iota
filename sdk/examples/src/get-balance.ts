// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { requestIotaFromFaucetV1, getFaucetHost } from '@iota/iota-sdk/faucet';

/**
 * Example: Client Read Operations
 *
 * This example demonstrates basic read operations using the IotaClient,
 * such as fetching an account's balance and coin objects from devnet.
 */
console.log('Setting up client and keypair...');

// Create a client connected to devnet
const client = new IotaClient({ url: getFullnodeUrl('devnet') });

// Generate a new Ed25519 keypair (for demo only)
const keypair = new Ed25519Keypair();
const address = keypair.toIotaAddress();
console.log(`Generated address: ${address}`);

console.log('Requesting IOTA from faucet...');

// Request IOTA from devnet faucet
await requestIotaFromFaucetV1({
    host: getFaucetHost('devnet'),
    recipient: address,
});

console.log(`Fetching balance for address: ${address}`);

// Get total balance for IOTA
const balance = await client.getBalance({ owner: address });
console.log('Total IOTA balance:', balance);

console.log('Fetching coin objects...');

// Get coin objects (limited to 5 for brevity)
const coins = await client.getCoins({ owner: address, limit: 5 });
console.log('Coin objects:', coins.data);
