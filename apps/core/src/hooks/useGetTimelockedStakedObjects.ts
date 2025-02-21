// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetTimelockedStakedObjects(address: string) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['get-timelocked-staked-objects', address],
        queryFn: () =>
            client.getTimelockedStakes({
                owner: address,
            }),
        enabled: !!address,
    });
}
