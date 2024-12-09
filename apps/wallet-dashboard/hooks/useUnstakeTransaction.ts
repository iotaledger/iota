// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { createTimelockedUnstakeTransaction, createUnstakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export type UseUnstakeTransactionParams =
    | {
          stakedIotaId: string;
          senderAddress: string;
      }
    | {
          groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
          senderAddress: string;
      };

export function useUnstakeTransaction({
    senderAddress,
    ...stakeOptions
}: UseUnstakeTransactionParams) {
    const client = useIotaClient();

    const isTimelockedUnstake = 'groupedTimelockedObjects' in stakeOptions;
    const timelockedObjectIds = isTimelockedUnstake
        ? stakeOptions.groupedTimelockedObjects.stakes.map((s) => s.timelockedStakedIotaId)
        : [];

    const queryKey = isTimelockedUnstake
        ? ['timelocked-unstake-transaction', timelockedObjectIds, senderAddress]
        : ['unstake-transaction', stakeOptions.stakedIotaId, senderAddress];

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey,
        queryFn: async () => {
            const transaction = isTimelockedUnstake
                ? createTimelockedUnstakeTransaction(timelockedObjectIds)
                : createUnstakeTransaction(stakeOptions.stakedIotaId);

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled:
            !!senderAddress &&
            (isTimelockedUnstake
                ? !!stakeOptions.groupedTimelockedObjects
                : !!stakeOptions.stakedIotaId),
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
