// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { createUnlockTimelockedObjectsTransaction } from '../utils';
import { useQuery } from '@tanstack/react-query';

export function useUnlockTimelockedObjects(address: string, objectIds: string[]) {
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['unlock-timelocked-objects', address, objectIds],
        queryFn: async () => {
            const transaction = createUnlockTimelockedObjectsTransaction({ address, objectIds });
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
