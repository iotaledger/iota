// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { createUnlockTimelockedObjectsTransaction } from '../utils';
import { useQuery } from '@tanstack/react-query';
import { useMaxTransactionSizeBytes } from './useMaxTransactionSizeBytes';
import { useGetClockTimestamp } from './useGetClockTimestamp';

export function useUnlockTimelockedObjectsTransaction(address: string, objectIds: string[]) {
    const client = useIotaClient();
    const { data: maxSizeBytes = Infinity } = useMaxTransactionSizeBytes();
    const { data: clockTimestampMs } = useGetClockTimestamp();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['unlock-timelocked-objects', address, objectIds],
        queryFn: async () => {
            const transaction = createUnlockTimelockedObjectsTransaction({
                address,
                objectIds,
                isClockTimestampEnabled: !!clockTimestampMs,
            });
            transaction.setSender(address);
            await transaction.build({ client, maxSizeBytes });
            return transaction;
        },
        enabled: !!address && !!objectIds?.length,
        gcTime: 0,
        select: (transaction) => {
            return {
                transactionBlock: transaction,
            };
        },
    });
}
