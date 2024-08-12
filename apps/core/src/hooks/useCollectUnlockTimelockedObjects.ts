// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { createUnlockAllTimelockedObjectsTransaction } from '../utils';
import { useQuery } from '@tanstack/react-query';

export function useCollectUnlockTimelockedObjects(address: string, objectIds: string[]) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['unlock-all-timelocked-objects-transaction', address, objectIds],
        queryFn: async () => {
            const transaction = createUnlockAllTimelockedObjectsTransaction({ address, objectIds });
            transaction.setSender(address);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!address && !!objectIds,
        gcTime: 0,
        select: (transaction) => {
            return {
                transactionBlock: transaction,
            };
        },
    });
}
