// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { queryKey } from './queryKey';

export function useGetPublicName(address: string) {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: [...queryKey.publicName(address)],
        async queryFn() {
            const publicName = await iotaNamesClient.getPublicName(address);

            return publicName;
        },
        enabled: !!iotaNamesClient && address.length > 0,
    });
}
