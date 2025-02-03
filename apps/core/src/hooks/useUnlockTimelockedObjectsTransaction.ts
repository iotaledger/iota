// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { createUnlockTimelockedObjectsTransaction } from '../utils';
import { useQuery } from '@tanstack/react-query';
import { Transaction } from '@iota/iota-sdk/transactions';

interface UnlockResult {
    transactionBlock: Transaction;
    validCount: number;
}

export function useUnlockTimelockedObjectsTransaction(address: string, objectIds: string[]) {
    const client = useIotaClient();

    return useQuery({
        queryKey: ['unlock-timelocked-objects', address, objectIds],
        queryFn: async (): Promise<UnlockResult> => {
            // Start with the full list.
            let currentLimit = objectIds.length;
            let isMaxSizeError = false;

            // Loop until we either succeed or run out of objects.
            do {
                // Use only the first currentLimit objectIds.
                const currentObjectIds = objectIds.slice(0, currentLimit);

                // Create a transaction for the current subset.
                const transaction = createUnlockTimelockedObjectsTransaction({
                    address,
                    objectIds: currentObjectIds,
                });

                transaction.setSender(address);

                try {
                    await transaction.build({ client, maxSizeBytes: 32 });
                    isMaxSizeError = false;
                    return {
                        transactionBlock: transaction,
                        validCount: currentLimit,
                    };
                } catch (e: unknown) {
                    if (
                        e instanceof Error &&
                        e.message.includes(
                            'Attempting to serialize to BCS, but buffer does not have enough size.',
                        )
                    ) {
                        isMaxSizeError = true;
                        // Reduce the currentLimit by one and try again.
                        currentLimit -= 1;
                    } else {
                        // If it's any other error, rethrow it.
                        throw e;
                    }
                }
            } while (isMaxSizeError && currentLimit > 0);

            // If we have reduced to zero, no valid transaction can be built.
            throw new Error('Unable to build transaction with any object count.');
        },
        enabled: !!address && objectIds.length > 0,
        gcTime: 0,
        // You could use select here if you want to massage the returned data further.
    });
}
