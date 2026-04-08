// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaNamesClient } from '../contexts';
import { useQuery } from '@tanstack/react-query';
import { normalizeIotaName } from '@iota/iota-names-sdk';

export function useGetDefaultIotaName(
    address: string | null | undefined,
    normalized: boolean = true,
) {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: ['iota-name', 'default-name', address, normalized],
        queryFn: async () => {
            if (!address) return null;

            const defaultName = await iotaNamesClient?.getPublicName(address);

            if (!defaultName) return null;

            return normalized ? normalizeIotaName(defaultName) : defaultName;
        },
        enabled: !!iotaNamesClient && !!address,
        staleTime: 1000 * 60 * 5,
    });
}
