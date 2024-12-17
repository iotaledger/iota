// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetCurrentEpochEndTimestamp() {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['current-epoch-end-timestamp'],
        queryFn: async () => {
            const iotaSystemState = await client.getLatestIotaSystemState();
            const epochStart = parseInt(iotaSystemState.epochStartTimestampMs);
            const epochDuration = parseInt(iotaSystemState.epochDurationMs);
            const epochEnd = epochStart + epochDuration;
            return epochEnd;
        },
        staleTime: 10 * 60 * 1000, // 10 minutes
    });
}
