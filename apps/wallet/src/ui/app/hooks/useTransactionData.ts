// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { TransactionBlock } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

export function useTransactionData(sender?: string | null, transaction?: TransactionBlock | null) {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['transaction-data', transaction?.serialize()],
        queryFn: async () => {
            const clonedTransaction = new TransactionBlock(transaction!);
            if (sender) {
                clonedTransaction.setSenderIfNotSet(sender);
            }
            // Build the transaction to bytes, which will ensure that the transaction data is fully populated:
            await clonedTransaction!.build({ client });
            return clonedTransaction!.blockData;
        },
        enabled: !!transaction,
    });
}

export function useTransactionGasBudget(
    sender?: string | null,
    transaction?: TransactionBlock | null,
) {
    const { data, ...rest } = useTransactionData(sender, transaction);

    const [formattedGas] = useFormatCoin(data?.gasConfig.budget, IOTA_TYPE_ARG);

    return {
        data: formattedGas,
        ...rest,
    };
}
