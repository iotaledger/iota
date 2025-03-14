// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createTimelockedStakeTransaction,
    getGasSummary,
    GroupedTimelockObject,
    useMaxTransactionSizeBytes,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function getAmountFromGroupedTimelockObjects(
    groupedTimelockObjects: GroupedTimelockObject[],
): bigint {
    return groupedTimelockObjects.reduce(
        (acc, obj) => acc + (obj.totalLockedAmount - (obj.splitAmount ?? 0n)),
        0n,
    );
}

export function useNewStakeTimelockedTransaction(
    validator: string,
    senderAddress: string,
    groupedTimelockObjects: GroupedTimelockObject[],
) {
    const amount = getAmountFromGroupedTimelockObjects(groupedTimelockObjects);
    const client = useIotaClient();
    const { data: maxTxSizeBytes = Infinity } = useMaxTransactionSizeBytes();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'stake-timelocked-transaction',
            validator,
            senderAddress,
            amount.toString(),
            groupedTimelockObjects.length,
        ],
        queryFn: async () => {
            const transaction = createTimelockedStakeTransaction(groupedTimelockObjects, validator);
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({
                client,
                maxSizeBytes: maxTxSizeBytes,
            });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                transaction,
                txDryRun,
            };
        },
        enabled: !!(validator && senderAddress && groupedTimelockObjects?.length),
        gcTime: 0,
        select: ({ transaction, txDryRun }) => {
            return {
                transaction,
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
