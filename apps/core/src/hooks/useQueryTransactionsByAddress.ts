// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export function useQueryTransactionsByAddress(address?: string | null) {
    const queryOptions = {
        showInput: true,
        showEffects: true,
        showEvents: true,
    };

    const rpc = useIotaClient();

    return useQuery({
        queryKey: ['transactions-by-address', address, queryOptions],
        queryFn: async () => {
            if (!address) {
                throw new Error('Address is required to query transactions.');
            }

            // combine from and to transactions
            const [toTxnIds, fromTxnIds] = await Promise.all([
                rpc.queryTransactionBlocks({
                    options: queryOptions,
                    filter: { ToAddress: address },
                }),
                rpc.queryTransactionBlocks({
                    options: queryOptions,
                    filter: { FromAddress: address },
                }),
            ]);

            const inserted = new Set();
            const uniqueList: IotaTransactionBlockResponse[] = [];

            [...toTxnIds.data, ...fromTxnIds.data]
                .sort((a, b) => Number(b.timestampMs ?? 0) - Number(a.timestampMs ?? 0))
                .forEach((txb) => {
                    if (inserted.has(txb.digest)) return;
                    uniqueList.push(txb);
                    inserted.add(txb.digest);
                });

            return uniqueList;
        },
        enabled: !!address,
        staleTime: 10 * 1000,
        refetchInterval: 20000,
    });
}
