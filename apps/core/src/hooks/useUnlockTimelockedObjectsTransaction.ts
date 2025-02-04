// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { createUnlockTimelockedObjectsTransaction } from '../utils';
import { useQuery } from '@tanstack/react-query';
import { Transaction } from '@iota/iota-sdk/transactions';
import { MAX_SIZE_BYTES_ERROR, useMaxTransactionSizeBytes } from './useMaxTransactionSizeBytes';

export interface UnlockAllSupplyIncrease {
    transactionBlock: Transaction;
    isMaxSizeReached: boolean;
    validCount: number;
}

export function useUnlockTimelockedObjectsTransaction(address: string, objectIds: string[]) {
    const client = useIotaClient();
    const { data: maxTxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        queryKey: ['unlock-timelocked-objects', address, objectIds],
        queryFn: async (): Promise<UnlockAllSupplyIncrease> => {
            let low = 0;
            let high = objectIds.length;
            let currentLimit = objectIds.length;
            let transaction: Transaction = {} as Transaction;
            let isSearchingOptimalLimit = false;

            // first attempt
            try {
                transaction = createUnlockTimelockedObjectsTransaction({
                    address,
                    objectIds: objectIds,
                });

                transaction.setSender(address);
                await transaction.build({ client, maxSizeBytes: maxTxSizeBytes });

                return {
                    transactionBlock: transaction,
                    isMaxSizeReached: false,
                    validCount: objectIds.length,
                };
            } catch (e: unknown) {
                if (e instanceof Error && isAttemptError(e)) {
                    isSearchingOptimalLimit = true;
                    console.info('Error max size. Start to search optimal count.');
                } else {
                    throw e;
                }
            }

            // if first attempt failed start to find optimal limit
            while (isSearchingOptimalLimit && low <= high) {
                try {
                    currentLimit = Math.ceil((low + high) / 2);
                    const currentObjectIds = objectIds.slice(0, currentLimit);

                    transaction = createUnlockTimelockedObjectsTransaction({
                        address,
                        objectIds: currentObjectIds,
                    });
                    transaction.setSender(address);
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
                transactionBlock: transaction,
                isMaxSizeReached: true,
                validCount: currentLimit,
            };
        },
        enabled: !!address && objectIds.length > 0,
        gcTime: 0,
    });
}

function isAttemptError(e: Error) {
    return e.message.includes(MAX_SIZE_BYTES_ERROR);
}
