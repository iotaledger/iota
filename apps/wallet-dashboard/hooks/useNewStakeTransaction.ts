// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';
import {
    createStakeTransaction,
    createTimelockedStakeTransaction,
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    VestingObject,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { useGetCurrentEpochStartTimestamp } from './useGetCurrentEpochStartTimestamp';

export function useNewStakeTransaction(
    validator: string,
    amount: bigint,
    senderAddress: string,
    isTimelockedStaking?: boolean,
) {
    let vestingObjects: VestingObject[] = [];
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    if (isTimelockedStaking && timelockedObjects && currentEpochMs) {
        vestingObjects = prepareObjectsForTimelockedStakingTransaction(
            timelockedObjects,
            amount,
            currentEpochMs,
        );
    }
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = isTimelockedStaking
                ? createTimelockedStakeTransaction(vestingObjects, validator)
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
