// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetCurrentEpochStartTimestamp() {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['current-epoch-start-timestamp'],
        queryFn: async () => {
            const iotaSystemState = await client.getLatestIotaSystemState();
            return parseInt(iotaSystemState.epochStartTimestampMs);
        },
        staleTime: 10 * 60 * 1000, // 10 minutes
    });
}
