// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createStakeTransaction } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewStakeTransaction(amount: string, validator: string, senderAddress: string) {
    const client = useIotaClient();

    const { data: transaction } = useQuery({
        queryKey: ['stake-transaction', amount, validator, senderAddress, client],
        queryFn: async () => {
            const transaction = createStakeTransaction(BigInt(amount), validator);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
    });

    const gasBudget = transaction?.blockData.gasConfig.budget?.toString() || '';
    return { transaction, gasBudget };
}
