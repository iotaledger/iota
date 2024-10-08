// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createTimelockedUnstakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useTimelockedUnstakeTransaction(
    timelockedStakedObjectIds: string[],
    senderAddress: string,
) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['timelocked-unstake-transaction', timelockedStakedObjectIds, senderAddress],
        queryFn: async () => {
            const transaction = createTimelockedUnstakeTransaction(timelockedStakedObjectIds);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!timelockedStakedObjectIds && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.blockData.gasConfig.budget,
            };
        },
    });
}
