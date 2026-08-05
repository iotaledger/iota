/** Copyright (c) 2026 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * This example shows how to publish a package containing `#[view]` functions
 * and call them with the TypeScript SDK.
 *
 * A function can only be called as a view if it is recorded in its module's
 * on-chain view functions metadata, which is recorded on devnet or a local
 * network, not yet on testnet or mainnet.
 *
 * By default it runs against devnet. Pass `--localnet` to mint and fund a
 * throwaway wallet from the local faucet, or `--devnet` / `--testnet` to use the
 * wallet configured in the `iota` CLI (assumed funded — the public faucets have
 * no HTTP API):
 *
 *   npm run call-view-function -- --localnet
 */

import { IotaClient } from '@iota/iota-sdk/client';
import { requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { loadConfiguredKeypair, publishViewFunctionsPackage } from '../utils';

type Network = 'localnet' | 'devnet' | 'testnet';

const RPC_URLS: Record<Network, string> = {
    localnet: 'http://127.0.0.1:9000',
    devnet: 'https://api.devnet.iota.cafe',
    testnet: 'https://api.testnet.iota.cafe',
};

const LOCAL_FAUCET_HOST = 'http://127.0.0.1:9123';

function parseNetwork(): Network {
    const flag = process.argv.slice(2).find((arg) => arg.startsWith('--'));
    switch (flag) {
        case undefined:
        case '--devnet':
            return 'devnet';
        case '--localnet':
            return 'localnet';
        case '--testnet':
            return 'testnet';
        default:
            throw new Error(`unknown flag ${flag}; use --localnet, --devnet, or --testnet`);
    }
}

async function run() {
    const network = parseNetwork();
    const iotaClient = new IotaClient({ url: RPC_URLS[network] });

    // On localnet, mint a throwaway wallet and fund it from the local faucet. On
    // devnet/testnet the public faucets have no HTTP API, so use the wallet
    // configured in the `iota` CLI and assume it is already funded.
    let keypair: Ed25519Keypair;
    if (network === 'localnet') {
        keypair = new Ed25519Keypair();
        await requestIotaFromFaucetV1({ host: LOCAL_FAUCET_HOST, recipient: keypair.toIotaAddress() });
    } else {
        keypair = loadConfiguredKeypair();
    }
    const sender = keypair.toIotaAddress();
    console.log(`Network: ${network}. Sender address: ${sender}`);

    // Wait for the wallet to hold a coin (funded by the faucet on localnet, or
    // already funded on a public network).
    while ((await iotaClient.getCoins({ owner: sender })).data.length === 0) {
        if (network !== 'localnet') {
            throw new Error(
                `Address ${sender} holds no coins on ${network}. Fund it at the ${network} ` +
                    `faucet website, then re-run.`,
            );
        }
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

    // The `vault` module is generic over the type `T` it stores. Build a shared
    // `Vault<Coin<IOTA>>` so a generic view has something to read.
    //
    // `create` takes the item by value, so the stored coin cannot be an existing
    // shared object. Instead, split a coin off the gas payment and hand that
    // split result straight to `create` in the same transaction.
    const createVaultTx = new Transaction();
    const [storedCoin] = createVaultTx.splitCoins(createVaultTx.gas, [1000]);
    createVaultTx.moveCall({
        target: `${packageId}::vault::create`,
        // `T = Coin<IOTA>`, the type argument the view must also be called with.
        typeArguments: ['0x2::coin::Coin<0x2::iota::IOTA>'],
        arguments: [
            storedCoin, // item: the coin to lock away
            createVaultTx.pure.u64(0), // unlock_at: timestamp, unused by `item`
            createVaultTx.pure.address(sender), // beneficiary
        ],
    });
    const { digest: createVaultDigest } = await iotaClient.signAndExecuteTransaction({
        transaction: createVaultTx,
        signer: keypair,
    });
    const createVaultResponse = await iotaClient.waitForTransaction({
        digest: createVaultDigest,
        options: { showObjectChanges: true },
    });
    const vault = createVaultResponse.objectChanges?.find(
        (change) => change.type === 'created' && change.objectType.includes('::vault::Vault<'),
    );
    if (vault?.type !== 'created') {
        throw new Error('Vault object not found');
    }
    const vaultId = vault.objectId;
    console.log(`Vault: ${vaultId}`);

    // Call the generic `vault::item` view, filling in the type argument
    // (`Coin<IOTA>`) and the function argument (the vault's object ID). The
    // returned coin arrives as a struct carrying its type and fields.
    const storedItem = await iotaClient.view({
        functionName: `${packageId}::vault::item`,
        typeArgs: ['0x2::coin::Coin<0x2::iota::IOTA>'],
        arguments: [vaultId],
    });
    if ('executionError' in storedItem) {
        throw new Error(`View call failed: ${storedItem.executionError}`);
    }
    console.log('Stored item:', JSON.stringify(storedItem.functionReturnValues[0], null, 2));
}

run().then(() => process.exit());
