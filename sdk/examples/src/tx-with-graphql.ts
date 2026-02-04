// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { getGraphQLUrl } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { requestIotaFromFaucetV1, getFaucetHost } from '@iota/iota-sdk/faucet';
import { IotaClientGraphQLTransport } from '@iota/graphql-transport';

/**
 * Example: Transaction with Client and GraphQL Transport
 *
 * This example demonstrates creating and executing a transaction (transfer IOTA).
 */
console.log('Setting up clients and keypair...');

const graphqlTransport = new IotaClientGraphQLTransport({
    url: getGraphQLUrl('devnet')!,
});

// IOTA client for devnet
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

console.log('Building a simple transaction (split gas coin)...');

// Build a minimal transaction: split gas coin
const tx = new Transaction();
const [coin] = tx.splitCoins(tx.gas, [1000]); // Split 1000 NANOS
tx.transferObjects([coin], address); // Transfer back to self

console.log('Signing and executing transaction...');

// Sign and execute
const result = await client.signAndExecuteTransaction({
    signer: keypair,
    transaction: tx,
});

console.log('Transaction executed. Digest:', result.digest);

// Wait for confirmation
await client.waitForTransaction({ digest: result.digest });
console.log('Transaction confirmed via RPC!');

console.log('Querying transaction via GraphQL...');

// GraphQL query to get transaction details
const query = `
  query GetTransaction($digest: String!) {
    transactionBlock(digest: $digest) {
      digest
      effects {
        status
      }
      sender {
        address
      }
    }
  }
`;

const graphqlResult = await graphqlTransport.graphqlQuery({
    query,
    variables: { digest: result.digest },
});

console.log('GraphQL response:', JSON.stringify(graphqlResult, null, 2));
