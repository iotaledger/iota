// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

import { ZkSendLink } from './claim.js';
import { getContractIds } from './zk-bag.js';
import type { ZkBagContractOptions } from './zk-bag.js';

export async function getSentTransactionsWithLinks({
    address,
    cursor,
    limit = 10,
    network = 'mainnet',
    contract = getContractIds(network),
    client = new IotaClient({ url: getFullnodeUrl(network) }),
    loadClaimedAssets = false,
    ...linkOptions
}: {
    address: string;
    contract?: ZkBagContractOptions;
    cursor?: string;
    limit?: number;
    network?: 'mainnet' | 'testnet';
    loadClaimedAssets?: boolean;

    // Link options:
    host?: string;
    path?: string;
    claimApi?: string;
    client?: IotaClient;
}) {
    const packageId = normalizeIotaAddress(contract.packageId);

    const page = await client.queryTransactionBlocks({
        filter: {
            FromAddress: address,
        },
        order: 'descending',
        cursor,
        limit,
        options: {
            showInput: true,
            showObjectChanges: true,
            showBalanceChanges: true,
        },
    });

    const data = await Promise.all(
        page.data.map(async (res) => {
            const transaction = res.transaction?.data.transaction;
            if (transaction?.kind !== 'ProgrammableTransaction') {
                throw new Error('Invalid transaction');
            }

            const newLinks = await Promise.all(
                transaction.transactions
                    .filter((tx) =>
                        'MoveCall' in tx
                            ? tx.MoveCall.package === packageId &&
                              tx.MoveCall.module === 'zk_bag' &&
                              tx.MoveCall.function === 'new'
                            : false,
                    )
                    .map(async (tx) => {
                        if (!('MoveCall' in tx)) {
                            throw new Error('Expected MoveCall');
                        }

                        const addressArg = tx.MoveCall.arguments?.[1];

                        if (
                            !addressArg ||
                            typeof addressArg !== 'object' ||
                            !('Input' in addressArg)
                        ) {
                            throw new Error('Invalid address argument');
                        }

                        const input = transaction.inputs[addressArg.Input];

                        if (input.type !== 'pure') {
                            throw new Error('Expected Address input to be a Pure value');
                        }

                        const address = normalizeIotaAddress(input.value as string);

                        const link = new ZkSendLink({
                            network,
                            address,
                            contract,
                            isContractLink: true,
                            client,
                            ...linkOptions,
                        });

                        await link.loadAssets({
                            transaction: res,
                            loadClaimedAssets,
                        });

                        return link;
                    }),
            );

            const regeneratedLinks = await Promise.all(
                transaction.transactions
                    .filter((tx) =>
                        'MoveCall' in tx
                            ? tx.MoveCall.package === packageId &&
                              tx.MoveCall.module === 'zk_bag' &&
                              tx.MoveCall.function === 'update_receiver'
                            : false,
                    )
                    .map(async (tx) => {
                        if (!('MoveCall' in tx)) {
                            throw new Error('Expected MoveCall');
                        }

                        const addressArg = tx.MoveCall.arguments?.[2];

                        if (
                            !addressArg ||
                            typeof addressArg !== 'object' ||
                            !('Input' in addressArg)
                        ) {
                            throw new Error('Invalid address argument');
                        }

                        const input = transaction.inputs[addressArg.Input];

                        if (input.type !== 'pure') {
                            throw new Error('Expected Address input to be a Pure value');
                        }

                        const address = normalizeIotaAddress(input.value as string);

                        const link = new ZkSendLink({
                            network,
                            address,
                            contract,
                            isContractLink: true,
                            ...linkOptions,
                        });

                        await link.loadAssets({ loadClaimedAssets });

                        return link;
                    }),
            );

            return {
                transaction: res,
                links: [...newLinks, ...regeneratedLinks],
            };
        }),
    );

    return {
        data,
        nextCursor: page.nextCursor,
        hasNextPage: page.hasNextPage,
    };
}
