// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, IotaClient, Network } from '@iota/iota-sdk/client';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

import { IotaNamesClient } from '../src/iota-names-client.js';
import { IotaNamesTransaction } from '../src/iota-names-transaction.js';

const COUPONS = ['PERCENT_20'];

(async () => {
    const name = `the-couponer-${Math.floor(Math.random() * 10000)}.iota`;

    const network = Network.Devnet;

    const { url, graphql, faucet } = getNetwork(network);

    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    console.log('[Secret key]:', keypair.getSecretKey());
    console.log('[Address]:', address);

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

    console.log('[Name Record (null if available)]: ', await iotaNamesClient.getNameRecord(name));
    console.log('Purchasing name:', name);

    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    const [coin] = iotaNamesTx.transaction.splitCoins(tx.gas, [10_000_000]);

    const nft = await iotaNamesTx.register({
        name,
        coin,
        couponCodes: COUPONS,
    });

    iotaNamesTx.transaction.transferObjects([nft], address);
    iotaNamesTx.transaction.transferObjects([coin], address);

    iotaNamesTx.transaction.setSender(address);

    const transaction = await iotaNamesTx.transaction.build({
        client: iotaClient,
    });

    const executedTransactionResponse = await iotaClient.signAndExecuteTransaction({
        transaction,
        signer: keypair,
    });

    console.log('[Transaction digest]: ', executedTransactionResponse.digest);

    await iotaClient.waitForTransaction({
        digest: executedTransactionResponse.digest,
    });

    const transactionBlockResponse = await iotaClient.getTransactionBlock({
        digest: executedTransactionResponse.digest,
        options: {
            showEffects: true,
        },
    });

    console.log('[Transaction response]:', transactionBlockResponse);
    console.log(`[IotaNames]: You own '${name}'`);
})();
