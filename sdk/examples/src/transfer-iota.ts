// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { requestIotaFromFaucetV1, getFaucetHost } from '@iota/iota-sdk/faucet';

/**
 * Example: Transaction Building and Execution
 *
 * This example demonstrates creating a transaction to transfer IOTA tokens
 * on the devnet. It generates a new keypair, requests IOTA from the faucet,
 * builds a transfer transaction, and executes it.
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

console.log('Building transfer transaction...');

// Build a transaction to transfer 1000 NANOS (0.001 IOTA) to a recipient
const recipient = '0x14abc6dfbf9ae91106ccc21b1a7839704cc9932a8ab571b7f60a2894cea219e7'; // Dummy recipient for demo
const tx = new Transaction();
const [coin] = tx.splitCoins(tx.gas, [1000]);
tx.transferObjects([coin], recipient);

console.log('Signing and executing transaction...');

// Sign and execute the transaction
const result = await client.signAndExecuteTransaction({
    signer: keypair,
    transaction: tx,
});

console.log('Transaction executed. Digest:', result.digest);

// Wait for confirmation
await client.waitForTransaction({ digest: result.digest });
console.log('Transaction confirmed!');
