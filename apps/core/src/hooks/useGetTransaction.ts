// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';

export function useGetTransaction(
    transactionId: string,
): UseQueryResult<IotaTransactionBlockResponse, Error> {
    const client = useIotaClient();
    return useQuery<IotaTransactionBlockResponse, Error>({
        queryKey: ['transactions-by-id', transactionId],
        queryFn: async () =>
            client.getTransactionBlock({
                digest: transactionId,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                    showBalanceChanges: true,
                    showObjectChanges: true,
                },
            }),
        enabled: !!transactionId,
    });
}
