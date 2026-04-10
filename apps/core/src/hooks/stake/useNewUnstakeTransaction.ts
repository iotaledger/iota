// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createUnstakeTransaction, getGasSummary } from '../../utils';
import { getUserFriendlyDryRunExecutionError } from '../../utils/formatUIErrors';
import { Transaction } from '@iota/iota-sdk/transactions';

export function useNewUnstakeTransaction(senderAddress: string, unstakeIotaId: string) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['unstake-transaction', unstakeIotaId, senderAddress],
        queryFn: async () => {
            const transaction = createUnstakeTransaction(unstakeIotaId);
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            if (txDryRun.effects.status.status !== 'success') {
                const errorText = txDryRun.effects.status.error || 'Transaction dry run failed';
                throw new Error(getUserFriendlyDryRunExecutionError(errorText));
            }
            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!(senderAddress && unstakeIotaId),
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
