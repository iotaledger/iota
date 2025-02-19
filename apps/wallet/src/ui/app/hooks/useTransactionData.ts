// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { Transaction, type TransactionData } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

export function useTransactionData(sender?: string | null, transaction?: Transaction | null) {
    const client = useIotaClient();
    return useQuery<TransactionData>({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['transaction-data', transaction?.getData()],
        queryFn: async () => {
            const clonedTransaction = Transaction.from(transaction!);
            if (sender) {
                clonedTransaction.setSenderIfNotSet(sender);
            }
            // Build the transaction to bytes, which will ensure that the transaction data is fully populated:
            await clonedTransaction!.build({ client });
            return clonedTransaction!.getData();
        },
        enabled: !!transaction,
    });
}
