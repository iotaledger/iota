// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createTimelockedUnstakeTransaction,
    getGasSummary,
    useMaxTransactionSizeBytes,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNewUnstakeTimelockedTransaction(
    senderAddress: string,
    timelockedUnstakeIotaIds: string[],
) {
    const client = useIotaClient();
    const { data: maxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['timelocked-unstake-transaction', timelockedUnstakeIotaIds, senderAddress],
        queryFn: async () => {
            const transaction = createTimelockedUnstakeTransaction(timelockedUnstakeIotaIds);
            transaction.setSender(senderAddress);
            await transaction.build({ client, maxSizeBytes });
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                transaction,
                txDryRun,
            };
        },
        enabled: !!(senderAddress && timelockedUnstakeIotaIds?.length),
        gcTime: 0,
        select: ({ transaction, txDryRun }) => {
            return {
                transaction,
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
