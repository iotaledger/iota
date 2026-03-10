// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, IotaClient, Network } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, NANOS_PER_IOTA, normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { expect } from 'vitest';

import { ALLOWED_METADATA, IotaNamesClient, IotaNamesTransaction } from '../src/index.js';

export const e2eLiveNetworkDryRunFlow = async (network_id: Network) => {
    let network = getNetwork(network_id);
    const client = new IotaClient({ url: network.url });
    let graphQlClient = new IotaGraphQLClient({
        url: network.graphql!,
    });

    let sender;
    if (network.id === Network.Mainnet) {
        sender = normalizeIotaAddress(
            '0xbb9ae3e92fd3020d8cfe76cc954da4dbd2b20f39c7077e4b4109410e8f126057',
        );
    } else {
        sender = normalizeIotaAddress('0x2');
    }
    const iotaNamesClient = new IotaNamesClient({
        graphQlClient,
        network: network.id,
    });

    // Getting price lists accurately
    const priceList = await iotaNamesClient.getPriceList();
    const renewalPriceList = await iotaNamesClient.getRenewalPriceList();

    // Expected lists
    let expectedPriceList;
    if (network.id === Network.Mainnet) {
        expectedPriceList = new Map([
            [[3, 3], 1500000000000],
            [[4, 4], 250000000000],
            [[5, 63], 50000000000],
        ]);
    } else if (network.id === Network.Testnet) {
        expectedPriceList = new Map([
            [[3, 3], 500000000000],
            [[4, 4], 250000000000],
            [[5, 63], 50000000000],
        ]);
    } else {
        expectedPriceList = new Map([
            [[3, 3], 500000000],
            [[4, 4], 100000000],
            [[5, 63], 10000000],
        ]);
    }

    let expectedRenewalPriceList;
    if (network.id === Network.Mainnet) {
        expectedRenewalPriceList = new Map([
            [[3, 3], 1500000000000],
            [[4, 4], 250000000000],
            [[5, 63], 50000000000],
        ]);
    } else if (network.id === Network.Testnet) {
        expectedRenewalPriceList = new Map([
            [[3, 3], 500000000000],
            [[4, 4], 250000000000],
            [[5, 63], 50000000000],
        ]);
    } else {
        expectedRenewalPriceList = new Map([
            [[3, 3], 150000000],
            [[4, 4], 50000000],
            [[5, 63], 5000000],
        ]);
    }

    expect(priceList).toEqual(expectedPriceList);
    expect(renewalPriceList).toEqual(expectedRenewalPriceList);

    const tx = new Transaction();

    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);

    const uniqueName =
        (Date.now().toString(36) + Math.random().toString(36).substring(2)).repeat(2) + '.iota';

    let gas;
    if (network.id === Network.Mainnet) {
        gas = 500n;
    } else if (network.id === Network.Testnet) {
        gas = 250n;
    } else {
        gas = 6n;
    }
    const [coinInput] = iotaNamesTx.transaction.splitCoins(iotaNamesTx.transaction.gas, [
        gas * NANOS_PER_IOTA,
    ]);
    // register random name like mclsl9pbdg8324x154cmclsl9pbdg8324x154c.iota for 2 years.
    const nft = await iotaNamesTx.register({
        name: uniqueName,
        coinConfig: { type: IOTA_TYPE_ARG },
        coin: coinInput,
    });
    // Sets the target address of the NFT.
    iotaNamesTx.setTargetAddress({
        nft,
        address: sender,
        isSubname: false,
    });

    iotaNamesTx.setPublic(uniqueName);

    // Sets the avatar of the NFT.
    iotaNamesTx.setUserData({
        nft,
        key: ALLOWED_METADATA.avatar,
        value: '0x0',
    });

    iotaNamesTx.setUserData({
        nft,
        key: ALLOWED_METADATA.ipfs,
        value: '0x1',
    });

    const subNft = iotaNamesTx.createSubname({
        parentNft: nft,
        name: 'node-subname.' + uniqueName,
        expirationTimestampMs: Date.now() + 1000 * 60 * 60 * 24 * 30,
        allowChildCreation: true,
        allowTimeExtension: true,
    });

    // create/remove some leaf names as an NFT
    iotaNamesTx.createLeafSubname({
        parentNft: nft,
        name: 'leaf.' + uniqueName,
        targetAddress: sender,
    });
    iotaNamesTx.removeLeafSubname({ parentNft: nft, name: 'leaf.' + uniqueName });

    // do it for sub nft too
    iotaNamesTx.createLeafSubname({
        parentNft: subNft,
        name: 'leaf.node-subname.' + uniqueName,
        targetAddress: sender,
    });
    iotaNamesTx.removeLeafSubname({ parentNft: subNft, name: 'leaf.node-subname.' + uniqueName });

    // extend expiration a bit further for the subNft
    iotaNamesTx.extendExpiration({
        nft: subNft,
        expirationTimestampMs: Date.now() + 1000 * 60 * 60 * 24 * 30 * 2,
    });

    iotaNamesTx.editSetup({
        parentNft: nft,
        name: 'node-subname.' + uniqueName,
        allowChildCreation: true,
        allowTimeExtension: false,
    });

    // let's go 2 levels deep and edit setups!
    const moreNestedNft = iotaNamesTx.createSubname({
        parentNft: subNft,
        name: 'more.node-subname.' + uniqueName,
        allowChildCreation: true,
        allowTimeExtension: true,
        expirationTimestampMs: Date.now() + 1000 * 60 * 60 * 24 * 30,
    });

    iotaNamesTx.editSetup({
        parentNft: subNft,
        name: 'more.node-subname.' + uniqueName,
        allowChildCreation: false,
        allowTimeExtension: false,
    });

    // do it for sub nft too
    tx.transferObjects([moreNestedNft, subNft, nft, coinInput], tx.pure.address(sender));

    tx.setSender(sender);

    if (network.id === Network.Mainnet) {
        tx.setGasPayment([
            {
                objectId: '0xc807394d03a93089862c9d9fd53235ddef034539284e821f093a968fe8868ff7',
                version: '1',
                digest: 'BoPYYfhsmbW3h45FENf6QLfZoQ87vohZFVTqr9yojGpa',
            },
            {
                objectId: '0x13ef83511595ac52c68b3575ee104b1a17d1bb990b8fa1495930721b18a3aa3c',
                version: '1',
                digest: '6R7Z1SZMixCZtZpxjLoML1k3utWQs6uJ1bEFve1rqsfK',
            },
        ]);
    } else if (network.id === Network.Testnet) {
        tx.setGasPayment([
            {
                objectId: '0x87d3011396b68cb019b9a539b368e1d57143a89ec8114d6d990ccb8c348ecc3e',
                version: '499614772',
                digest: '2F1DFbzvZH5Mb1E9VzuGzQfV7aJt3DvnDnf8w1QvhvAr',
            },
        ]);
    } else if (network.id === Network.Devnet) {
        tx.setGasPayment([
            {
                objectId: '0x7a8610b11e0bfe5231a7aca06e0ef0321ef0afa611195ab35ba6badf7a538921',
                version: '42',
                digest: 'GgP7Fdc2j7RPgwsuKppUzHeZeSVR7NE9hR8tiGWNtLrE',
            },
        ]);
    }

    return client.dryRunTransactionBlock({
        transactionBlock: await tx.build({
            client,
        }),
    });
};
