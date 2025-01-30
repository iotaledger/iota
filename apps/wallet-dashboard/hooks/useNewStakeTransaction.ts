// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createStakeTransaction,
    createTimelockedStakeTransaction,
    GroupedTimelockObject,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewStakeTransaction(validator: string, amount: bigint, senderAddress: string) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = createStakeTransaction(amount, validator);
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

export function getAmountFromGroupedTimelockObjects(
    groupedTimelockObjects: GroupedTimelockObject[],
): bigint {
    return groupedTimelockObjects.reduce(
        (acc, obj) => acc + (obj.totalLockedAmount - (obj.splitAmount ?? 0n)),
        0n,
    );
}

export function useNewStakeTimelockedTransaction(
    validator: string,
    senderAddress: string,
    groupedTimelockObjects: GroupedTimelockObject[],
) {
    const amount = getAmountFromGroupedTimelockObjects(groupedTimelockObjects);
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'stake-timelocked-transaction',
            validator,
            senderAddress,
            amount.toString(),
            groupedTimelockObjects.length,
        ],
        queryFn: async () => {
            const transaction = createTimelockedStakeTransaction(groupedTimelockObjects, validator);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!(validator && senderAddress && groupedTimelockObjects?.length),
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
