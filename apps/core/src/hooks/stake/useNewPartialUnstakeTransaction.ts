// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createPartialUnstakeTransaction, getGasSummary } from '../../utils';
import { Transaction } from '@iota/iota-sdk/transactions';

export function useNewPartialUnstakeTransaction(
    senderAddress: string,
    unstakeIotaId: string,
    unstakeAmount: bigint,
) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'partial-unstake-transaction',
            unstakeIotaId,
            unstakeAmount.toString(),
            senderAddress,
        ],
        queryFn: async () => {
            const transaction = createPartialUnstakeTransaction(unstakeIotaId, unstakeAmount);
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!(senderAddress && unstakeIotaId && unstakeAmount > 0n),
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
