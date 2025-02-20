// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useIotaClient } from '@iota/dapp-kit';
import { type Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

export function useTransactionDryRunWithoutSigner(transaction: Transaction, sender: string) {
    const client = useIotaClient();
    const response = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['useTxDryRun', transaction.getData(), sender],
        queryFn: async () => {
            transaction.setSenderIfNotSet(sender);
            const txBytes = await transaction.build({ client });
            return client.dryRunTransactionBlock({ transactionBlock: txBytes });
        },
        enabled: !!transaction,
    });
    return response;
}
