// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createTimelockedUnstakeTransaction, createUnstakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

type UnstakeTypeTransaction =
    | {
          unstakeIotaId: string;
          isTimelockedUnstake?: never | false;
          unstakeIotaIds?: never;
      }
    | {
          unstakeIotaId?: never;
          unstakeIotaIds: string[];
          isTimelockedUnstake: true;
      };

interface UseUnstakeTransactionBaseParams {
    senderAddress: string;
}

export type UseUnstakeTransactionParams = UseUnstakeTransactionBaseParams & UnstakeTypeTransaction;

export function useUnstakeTransaction({
    senderAddress,
    unstakeIotaId,
    unstakeIotaIds,
    isTimelockedUnstake,
}: UseUnstakeTransactionParams) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: isTimelockedUnstake
            ? ['timelocked-unstake-transaction', unstakeIotaIds, senderAddress]
            : ['unstake-transaction', unstakeIotaId, senderAddress],
        queryFn: async () => {
            const transaction = isTimelockedUnstake
                ? createTimelockedUnstakeTransaction(unstakeIotaIds)
                : createUnstakeTransaction(unstakeIotaId);

            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!(senderAddress && (unstakeIotaIds?.length || unstakeIotaId)),
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
