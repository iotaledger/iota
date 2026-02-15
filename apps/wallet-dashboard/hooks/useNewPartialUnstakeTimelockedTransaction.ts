// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createPartialTimelockedUnstakeTransaction,
    getGasSummary,
    useMaxTransactionSizeBytes,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewPartialUnstakeTimelockedTransaction(
    senderAddress: string,
    timelockedUnstakeIotaId: string,
    unstakeAmount: bigint,
) {
    const client = useIotaClient();
    const { data: maxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'partial-timelocked-unstake-transaction',
            timelockedUnstakeIotaId,
            unstakeAmount.toString(),
            senderAddress,
        ],
        queryFn: async () => {
            const transaction = createPartialTimelockedUnstakeTransaction(
                timelockedUnstakeIotaId,
                unstakeAmount,
            );
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client, maxSizeBytes });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                transaction,
                txDryRun,
            };
        },
        enabled: !!(senderAddress && timelockedUnstakeIotaId && unstakeAmount > 0n),
        gcTime: 0,
        select: ({ transaction, txDryRun }) => {
            return {
                transaction,
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
