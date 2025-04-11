// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

const MAX_TRANSACTIONS_PER_REQ = 50;

export function useEndOfEpochTransactionFromCheckpoint(
    checkpoinId?: string,
    limit = MAX_TRANSACTIONS_PER_REQ,
) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['end-of-epoch-transaction', checkpoinId],
        queryFn: async () => {
            let cursor: string | undefined | null = null;
            const filter = checkpoinId ? { Checkpoint: checkpoinId } : undefined;
            // keep fetching until cursor is null or undefined
            do {
                const { data: transactions, nextCursor } = await client.queryTransactionBlocks({
                    filter,
                    cursor,
                    order: 'descending',
                    limit,
                    options: {
                        showEffects: true,
                        showInput: true,
                        showEvents: true,
                    },
                });
                if (!transactions || !transactions.length) {
                    break;
                }

                const endOfEpochTransaction = transactions.filter(
                    (tx): tx is IotaTransactionBlockResponse =>
                        tx.transaction?.data.transaction.kind === 'EndOfEpochTransaction',
                );
                if (endOfEpochTransaction.length) {
                    return endOfEpochTransaction[0];
                }
                cursor = nextCursor;
            } while (cursor);
        },
        staleTime: 10 * 1000,
        enabled: !!checkpoinId,
    });
}
