// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createTimelockedUnstakeTransaction,
    createUnstakeTransaction,
    useMaxTransactionSizeBytes,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewUnstakeTransaction(senderAddress: string, unstakeIotaId: string) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['unstake-transaction', unstakeIotaId, senderAddress],
        queryFn: async () => {
            const transaction = createUnstakeTransaction(unstakeIotaId);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!(senderAddress && unstakeIotaId),
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}

export function useNewUnstakeTimelockedTransaction(
    senderAddress: string,
    timelockedUnstakeIotaIds: string[],
) {
    const client = useIotaClient();
    const { data: maxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['timelocked-unstake-transaction', timelockedUnstakeIotaIds, senderAddress],
        queryFn: async () => {
            const transaction = createTimelockedUnstakeTransaction(timelockedUnstakeIotaIds);
            transaction.setSender(senderAddress);
            await transaction.build({ client, maxSizeBytes });
            return transaction;
        },
        enabled: !!(senderAddress && timelockedUnstakeIotaIds?.length),
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
