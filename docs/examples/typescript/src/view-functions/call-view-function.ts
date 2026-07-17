/** Copyright (c) 2026 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * This example shows how to publish a package containing `#[view]` functions
 * and call them with the TypeScript SDK.
 *
 * A function can only be called as a view if it is recorded in its module's
 * on-chain view functions metadata. This requires the network to have view
 * function support enabled; at the time of writing this is not yet the case
 * on testnet, so run this example against a local network
 * (`iota-localnet start --force-regenesis --with-faucet`):
 *
 * npm run call-view-function
 */

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { publishViewFunctionsPackage } from '../utils';

async function run() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({ url: getFullnodeUrl('localnet') });

    // Generate a sender address and fund it from the local faucet.
    const keypair = new Ed25519Keypair();
    const sender = keypair.toIotaAddress();
    console.log(`Sender address: ${sender}`);
    await requestIotaFromFaucetV1({ host: getFaucetHost('localnet'), recipient: sender });
    while ((await iotaClient.getCoins({ owner: sender })).data.length === 0) {
        await new Promise((resolve) => setTimeout(resolve, 1000));
    }

    // Publish the `view_functions` example package.
    const packageId = await publishViewFunctionsPackage(iotaClient, keypair);

    // Create a shared, empty leaderboard.
    const createTx = new Transaction();
    createTx.moveCall({ target: `${packageId}::leaderboard::create` });
    const { digest: createDigest } = await iotaClient.signAndExecuteTransaction({
        transaction: createTx,
        signer: keypair,
    });
    const createResponse = await iotaClient.waitForTransaction({
        digest: createDigest,
        options: { showObjectChanges: true },
    });
    const leaderboard = createResponse.objectChanges?.find(
        (change) =>
            change.type === 'created' && change.objectType.endsWith('::leaderboard::Leaderboard'),
    );
    if (leaderboard?.type !== 'created') {
        throw new Error('Leaderboard object not found');
    }
    const leaderboardId = leaderboard.objectId;
    console.log(`Leaderboard: ${leaderboardId}`);

    // Record a score so the views have something to return.
    const submitTx = new Transaction();
    submitTx.moveCall({
        target: `${packageId}::leaderboard::submit_score`,
        arguments: [
            submitTx.object(leaderboardId),
            submitTx.pure.address(sender),
            submitTx.pure.u64(2500),
        ],
    });
    const { digest: submitDigest } = await iotaClient.signAndExecuteTransaction({
        transaction: submitTx,
        signer: keypair,
    });
    await iotaClient.waitForTransaction({ digest: submitDigest });

    // Call a view returning a primitive. `u64` values arrive as strings.
    const totalEntries = await iotaClient.view({
        functionName: `${packageId}::leaderboard::total_entries`,
        arguments: [leaderboardId],
    });
    if ('executionError' in totalEntries) {
        throw new Error(`View call failed: ${totalEntries.executionError}`);
    }
    console.log('Total entries:', totalEntries.functionReturnValues[0]);

    // Call a view returning `Option<ScoreEntry>`. `some` arrives as the
    // struct's type and fields, `none` as null.
    const highestScore = await iotaClient.view({
        functionName: `${packageId}::leaderboard::highest_score`,
        arguments: [leaderboardId],
    });
    if ('executionError' in highestScore) {
        throw new Error(`View call failed: ${highestScore.executionError}`);
    }
    console.log('Highest score:', JSON.stringify(highestScore.functionReturnValues[0], null, 2));
}

run().then(() => process.exit());
