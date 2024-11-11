// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

export function useMigrationTransaction(
    address: string,
    basicOutputObjects?: IotaObjectData[],
    nftOutputObjects?: IotaObjectData[],
) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['migration-transaction', address],
        queryFn: async () => {
            const transaction = new Transaction();
            transaction.setSender(address);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!address,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.getData().gasData.budget,
            };
        },
    });
}
