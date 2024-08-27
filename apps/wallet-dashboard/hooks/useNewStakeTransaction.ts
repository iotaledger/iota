// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createStakeTransaction,
    createTimelockedStakeTransaction,
    ExtendedTimelockObject,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewStakeTransaction(
    validator: string,
    amount: bigint,
    senderAddress: string,
    extendedTimelockObjects?: ExtendedTimelockObject[],
) {
    const isTimelockedStaking =
        extendedTimelockObjects?.length && extendedTimelockObjects?.length > 0;

    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = isTimelockedStaking
                ? createTimelockedStakeTransaction(extendedTimelockObjects, validator)
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
                gasBudget: transaction.blockData.gasConfig.budget,
            };
        },
    });
}
