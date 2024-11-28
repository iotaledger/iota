// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';
import { useTransactionSummary } from './useTransactionSummary';

export function useGetTransaction(
    transactionId: string,
    queryOptions?: { retry?: number; initialData?: IotaTransactionBlockResponse },
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
        retry: queryOptions?.retry,
        initialData: queryOptions?.initialData,
    });
}

export function useGetTransactionWithSummary(
    transactionDigest: string,
    currentAddress: string,
    initialData?: IotaTransactionBlockResponse,
    recognizedPackagesList: string[] = [],
) {
    const txResponse = useGetTransaction(transactionDigest, { retry: 8, initialData });

    const { data: transaction } = txResponse;

    const summary = useTransactionSummary({
        transaction,
        currentAddress,
        recognizedPackagesList,
    });

    return { summary, ...txResponse };
}
