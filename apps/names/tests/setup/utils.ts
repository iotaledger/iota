// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClientGraphQLTransport } from '@iota/graphql-transport';
import { IotaNamesClient } from '@iota/iota-names-sdk';
import { getNetwork, IotaClient } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

if (!process.env.ADMIN_MNEMONIC) {
    throw new Error('env ADMIN_MNEMONIC not set. Cannot setup tests.');
}

const adminKeypair = Ed25519Keypair.deriveKeypair(process.env.ADMIN_MNEMONIC);

const DEFAULT_NETWORK = process.env.NEXT_PUBLIC_NAMES_DAPP_DEFAULT_NETWORK as string;

if (!DEFAULT_NETWORK) {
    throw new Error(
        'NEXT_PUBLIC_NAMES_DAPP_DEFAULT_NETWORK is not defined in the environment variables',
    );
}

const NETWORK_CONFIG = getNetwork(DEFAULT_NETWORK);

const iotaClientGraphQl = new IotaClient({
    transport: new IotaClientGraphQLTransport({
        url: NETWORK_CONFIG.graphql!,
    }),
});

const client = new IotaClient({
    url: NETWORK_CONFIG.url,
});

const iotaNamesClient = new IotaNamesClient({
    graphQlClient: new IotaGraphQLClient({
        url: NETWORK_CONFIG.graphql!,
    }),
    network: NETWORK_CONFIG.id,
});

if (!iotaNamesClient.config) {
    throw new Error('IOTA Names Client Config is not properly configured');
}

const packageId = iotaNamesClient.getPackage('packageId', 'v1');
const paymentsPackageId = iotaNamesClient.getPackage('paymentsPackageId', 'v1');
const auctionPackageId = iotaNamesClient.getPackage('auctionPackageId', 'v1');
const iotaNamesObjectId = iotaNamesClient.getPackage('iotaNamesObjectId', 'v1');

const authKeyType = `${packageId}::iota_names::AuthKey`;

const PAYMENT_TYPE = `${authKeyType}<${paymentsPackageId}::payments::PaymentsAuth>`;
const AUCTION_TYPE = `${authKeyType}<${auctionPackageId}::auction::AuctionAuth>`;

async function getAuthorizedSmartContractTypes() {
    const { data: dynamicFields } = await iotaClientGraphQl.getDynamicFields({
        parentId: iotaNamesObjectId,
    });

    const fieldTypes = dynamicFields.map((field) => field.name.type);

    const isPaymentAuthorized = fieldTypes.includes(PAYMENT_TYPE);
    const isAuctionAuthorized = fieldTypes.includes(AUCTION_TYPE);

    return {
        isPaymentAuthorized,
        isAuctionAuthorized,
    };
}

export {
    DEFAULT_NETWORK,
    NETWORK_CONFIG,
    client,
    iotaNamesClient,
    PAYMENT_TYPE,
    AUCTION_TYPE,
    getAuthorizedSmartContractTypes,
    adminKeypair,
    iotaClientGraphQl,
};
