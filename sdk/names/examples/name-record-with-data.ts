// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, IotaClient, Network } from '@iota/iota-sdk/client';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

import { ALLOWED_METADATA } from '../src/constants.js';
import { IotaNamesClient } from '../src/iota-names-client.js';
import { IotaNamesTransaction } from '../src/iota-names-transaction.js';

(async () => {
    const name = `test-fields-${Math.floor(Math.random() * 10000000)}.iota`;

    const network = Network.Devnet;
    const { url, graphql, faucet } = getNetwork(network);

    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    console.log('[Address]:', address);
    console.log('[Name]:', name);

    // Get funds from faucet
    await requestIotaFromFaucetV0({
        host: faucet!,
        recipient: address,
    });
    console.log('[Faucet]: Received funds');

    const graphQlClient = new IotaGraphQLClient({ url: graphql! });
    const iotaClient = new IotaClient({
        url,
    });
    const iotaNamesClient = new IotaNamesClient({
        graphQlClient,
        network,
    });

    // Check if name is available
    console.log('[Name Record (before registration)]:', await iotaNamesClient.getNameRecord(name));

    // Step 1: Register the name
    console.log('\n=== STEP 1: Registering name ===');
    const registerTx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, registerTx);
    const [coin] = iotaNamesTx.transaction.splitCoins(registerTx.gas, [100_000_000]);

    const nft = await iotaNamesTx.register({
        name,
        years: 1,
        coin,
    });

    iotaNamesTx.transaction.transferObjects([nft], address);
    iotaNamesTx.transaction.transferObjects([coin], address);
    iotaNamesTx.transaction.setSender(address);

    const registerTransaction = await iotaNamesTx.transaction.build({ client: iotaClient });
    const registerResponse = await iotaClient.signAndExecuteTransaction({
        transaction: registerTransaction,
        signer: keypair,
    });

    console.log('[Registration digest]:', registerResponse.digest);
    await iotaClient.waitForTransaction({ digest: registerResponse.digest });

    // Get the NFT object ID from the transaction
    const registerTxResponse = await iotaClient.getTransactionBlock({
        digest: registerResponse.digest,
        options: { showEffects: true, showObjectChanges: true },
    });

    console.log('[Transaction response]:', JSON.stringify(registerTxResponse, null, 2));

    // Find the created object (NFT)
    const nftObject = registerTxResponse.objectChanges?.find(
        (change) =>
            change.type === 'created' &&
            (change.objectType.includes('RegistrationNFT') ||
                change.objectType.includes('iota_names')),
    );

    if (!nftObject || nftObject.type !== 'created') {
        console.log('[Available object changes]:', registerTxResponse.objectChanges);
        throw new Error('Could not find created NFT object');
    }

    const nftId = nftObject.objectId;
    console.log('[NFT ID]:', nftId);

    // Step 2: Set custom fields
    console.log('\n=== STEP 2: Setting custom fields ===');

    // Set avatar field
    const avatarTx = new Transaction();
    const avatarIotaNamesTx = new IotaNamesTransaction(iotaNamesClient, avatarTx);

    avatarIotaNamesTx.setUserData({
        nft: nftId,
        key: 'avatar',
        value: 'ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG',
    });

    avatarTx.setSender(address);
    const avatarTransaction = await avatarTx.build({ client: iotaClient });
    const avatarResponse = await iotaClient.signAndExecuteTransaction({
        transaction: avatarTransaction,
        signer: keypair,
    });

    console.log('[Avatar set digest]:', avatarResponse.digest);
    await iotaClient.waitForTransaction({ digest: avatarResponse.digest });

    // Set content_hash field
    const contentTx = new Transaction();
    const contentIotaNamesTx = new IotaNamesTransaction(iotaNamesClient, contentTx);

    contentIotaNamesTx.setUserData({
        nft: nftId,
        key: ALLOWED_METADATA.ipfs,
        value: '0x1234567890abcdef1234567890abcdef12345678',
    });

    contentTx.setSender(address);
    const contentTransaction = await contentTx.build({ client: iotaClient });
    const contentResponse = await iotaClient.signAndExecuteTransaction({
        transaction: contentTransaction,
        signer: keypair,
    });

    console.log('[Content hash set digest]:', contentResponse.digest);
    await iotaClient.waitForTransaction({ digest: contentResponse.digest });

    console.log('[Only avatar and content_hash are allowed metadata keys]');

    // Step 3: Query the name record to verify fields are set
    console.log('\n=== STEP 3: Querying name record with fields ===');
    const nameRecord = await iotaNamesClient.getNameRecord(name);

    console.log('[Final Name Record]:');
    console.log('- Name:', nameRecord?.name);
    console.log('- NFT ID:', nameRecord?.nftId);
    console.log('- Target Address:', nameRecord?.targetAddress);
    console.log('- Data fields:', nameRecord?.data);
    console.log('- Avatar:', nameRecord?.avatar);

    console.log('\n=== SUCCESS ===');
    console.log(`Name '${name}' registered and fields set successfully!`);
})();
