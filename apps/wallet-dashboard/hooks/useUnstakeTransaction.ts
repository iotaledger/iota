// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createUnstakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useUnstakeTransaction(stakedIotaId: string, senderAddress: string) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['unstake-transaction', stakedIotaId, senderAddress, client],
        queryFn: async () => {
            const transaction = createUnstakeTransaction(stakedIotaId);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!stakedIotaId && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.blockData.gasConfig.budget,
            };
        },
    });
}
