// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Move Authenticator (Account Abstraction) example.
 *
 * 1. Publishes the `account` Move package.
 * 2. Links the authenticator function via `account::link_auth`.
 * 3. Sends a transfer from the account-abstracted address.
 */

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { requestIotaFromFaucetV1, getFaucetHost } from '@iota/iota-sdk/faucet';
import { Transaction } from '@iota/iota-sdk/transactions';
import { MoveAuthenticatorBuilder, MoveSigner } from '@iota/iota-sdk/keypairs/move-authenticator';
import { bcs } from '@iota/iota-sdk/bcs';
import { execSync } from 'child_process';

const client = new IotaClient({ url: getFullnodeUrl('devnet') });
const keypair = new Ed25519Keypair();
const deployerAddress = keypair.toIotaAddress();

// Fund the deployer account
await requestIotaFromFaucetV1({
    host: getFaucetHost('devnet'),
    recipient: deployerAddress,
});

// --- 1. Publish the Move package ---

const { modules, dependencies } = JSON.parse(
    execSync('iota move build --dump-bytecode-as-base64 --path ./src/move-authenticator', {
        encoding: 'utf-8',
    }),
);

const publishTx = new Transaction();
const upgradeCap = publishTx.publish({ modules, dependencies });
publishTx.transferObjects([upgradeCap], deployerAddress);

const publishResult = await client.signAndExecuteTransaction({
    transaction: publishTx,
    signer: keypair,
    options: { showObjectChanges: true },
});
await client.waitForTransaction({ waitMode: 'checkpoint', digest: publishResult.digest });

function findObjectChange(predicate: (c: any) => boolean, label: string): any {
    const found = publishResult.objectChanges?.find(predicate);
    if (!found) throw new Error(`Failed to find ${label} in publish result`);
    return found;
}

const packageId = findObjectChange((c) => c.type === 'published', 'published package').packageId;
const accountId = findObjectChange(
    (c) => c.type === 'created' && c.objectType.endsWith('::account::Account'),
    'Account object',
).objectId;
const metadataId = findObjectChange(
    (c) => c.type === 'created' && c.objectType.includes('PackageMetadataV1'),
    'PackageMetadataV1 object',
).objectId;

console.log('Published package:', packageId);
console.log('Account object:  ', accountId);
console.log('Metadata object: ', metadataId);

// --- 2. Link the authenticator function ---

const linkTx = new Transaction();
linkTx.moveCall({
    target: `${packageId}::account::link_auth`,
    arguments: [
        linkTx.object(accountId),
        linkTx.object(metadataId),
        linkTx.pure.string('account'),
        linkTx.pure.string('authenticate'),
    ],
});

await client.signAndExecuteTransaction({
    transaction: linkTx,
    signer: keypair,
    options: { showEffects: true },
});
console.log('Authenticator linked.');

// --- 3. Send a transfer from the account-abstracted address ---

const authBuilder = new MoveAuthenticatorBuilder(accountId).addPure(
    bcs.string().serialize('hello').toBytes(),
);
const moveSigner = new MoveSigner(await authBuilder.finish(client));
const aaAddress = moveSigner.getPublicKey().toIotaAddress();
console.log('Account-abstracted address:', aaAddress);

await requestIotaFromFaucetV1({
    host: getFaucetHost('devnet'),
    recipient: aaAddress,
});

const recipient = '0x14abc6dfbf9ae91106ccc21b1a7839704cc9932a8ab571b7f60a2894cea219e7';
const transferTx = new Transaction();
const [coin] = transferTx.splitCoins(transferTx.gas, [1000]);
transferTx.transferObjects([coin], recipient);
transferTx.setSender(aaAddress);

const builtTx = await transferTx.build({ client });
const result = await client.signAndExecuteTransaction({
    signer: moveSigner,
    transaction: builtTx,
});

console.log('Transfer digest:', result.digest);
await client.waitForTransaction({ digest: result.digest });
console.log('Transfer confirmed.');
