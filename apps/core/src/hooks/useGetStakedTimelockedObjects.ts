// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetStakedTimelockedObjects(address: string) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['get-staked-timelocked-objects', address],
        queryFn: () =>
            client.getTimelockedStakes({
                owner: address,
            }),
        enabled: !!address,
    });
}
