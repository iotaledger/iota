// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetActiveValidatorsInfo() {
    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['get-active-validators-info'],
        queryFn: async () => {
            const iotaSystemState = await client.getLatestIotaSystemState();
            return iotaSystemState.activeValidators;
        },
        staleTime: 10 * 60 * 1000, // 10 minutes
    });
}
