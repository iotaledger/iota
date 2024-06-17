// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createStakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewStakeTransaction(validator: string, amount: string, senderAddress: string) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['stake-transaction', validator, amount, senderAddress, client],
        queryFn: async () => {
            const transaction = createStakeTransaction(BigInt(amount), validator);
            transaction?.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction?.blockData.gasConfig.budget,
            };
        },
    });
}
