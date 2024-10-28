// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createStakeTransaction,
    createTimelockedStakeTransaction,
    GroupedTimelockObject,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewStakeTransaction(
    validator: string,
    amount: bigint,
    senderAddress: string,
    isTimelockedStaking: boolean = false,
    groupedTimelockObjects?: GroupedTimelockObject[],
) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = isTimelockedStaking
                ? createTimelockedStakeTransaction(groupedTimelockObjects || [], validator)
                : createStakeTransaction(amount, validator);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
