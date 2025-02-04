// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createTimelockedUnstakeTransaction,
    createUnstakeTransaction,
    SIZE_LIMIT_EXCEEDED,
    useMaxTransactionSizeBytes,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

export interface UnstakeTimelockedTransactionResponse {
    transaction: Transaction;
    isMaxSizeReached: boolean;
}

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
    const { data: maxTxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['timelocked-unstake-transaction', timelockedUnstakeIotaIds, senderAddress],
        queryFn: async (): Promise<UnstakeTimelockedTransactionResponse> => {
            let low = 0;
            let high = timelockedUnstakeIotaIds.length;
            let currentLimit = timelockedUnstakeIotaIds.length;
            let transaction: Transaction = {} as Transaction;
            let isSearchingOptimalLimit = false;

            // first attempt
            try {
                transaction = createTimelockedUnstakeTransaction(timelockedUnstakeIotaIds);
                transaction.setSender(senderAddress);
                await transaction.build({ client, maxSizeBytes: maxTxSizeBytes });
                return {
                    transaction: transaction,
                    isMaxSizeReached: false,
                };
            } catch (e: unknown) {
                if (e instanceof Error && isAttemptError(e)) {
                    isSearchingOptimalLimit = true;
                    console.info('Error max size. Start to search optimal count.');
                } else {
                    throw e;
                }
            }

            while (isSearchingOptimalLimit && low <= high) {
                try {
                    currentLimit = Math.ceil((low + high) / 2);
                    const currentObjectIds = timelockedUnstakeIotaIds.slice(0, currentLimit);

                    transaction = createTimelockedUnstakeTransaction(currentObjectIds);
                    transaction.setSender(senderAddress);
                    await transaction.build({ client, maxSizeBytes: maxTxSizeBytes });

                    low = currentLimit + 1;
                } catch (e: unknown) {
                    if (e instanceof Error && isAttemptError(e)) {
                        high = currentLimit - 1;
                    } else {
                        throw e;
                    }
                }
            }

            return {
                transaction,
                isMaxSizeReached: true,
            };
        },
        enabled: !!(senderAddress && timelockedUnstakeIotaIds?.length),
        gcTime: 0,
        select: ({ transaction, isMaxSizeReached }) => {
            return {
                transaction,
                isMaxSizeReached,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}

function isAttemptError(e: Error) {
    return e.message.includes(SIZE_LIMIT_EXCEEDED);
}
