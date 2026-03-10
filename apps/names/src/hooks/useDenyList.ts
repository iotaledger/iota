// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { queryKey } from './queryKey';

export function useReservedList() {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: [...queryKey.reservedList()],
        queryFn: async () => {
            return await iotaNamesClient.getRestrictedList('reserved');
        },
        staleTime: 60 * 60 * 1000, // Cache for 1 hour
        enabled: !!iotaNamesClient,
    });
}

export function useBlockedList() {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: [...queryKey.blockedList()],
        queryFn: async () => {
            return await iotaNamesClient.getRestrictedList('blocked');
        },
        staleTime: 60 * 60 * 1000, // Cache for 1 hour
        enabled: !!iotaNamesClient,
    });
}
