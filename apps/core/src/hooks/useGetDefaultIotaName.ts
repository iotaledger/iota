// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';
import { useIotaNamesClient } from '../contexts';
import { useFeatureEnabledByNetwork } from './useFeatureEnabledByNetwork';
import { Feature } from '../enums';
import { useQuery } from '@tanstack/react-query';
import { normalizeIotaName } from '@iota/iota-names-sdk';

export function useGetDefaultIotaName(
    address: string | null | undefined,
    normalized: boolean = true,
) {
    const networkName = useNetwork();
    const network = getNetwork(networkName).id;

    const { iotaNamesClient } = useIotaNamesClient();
    const isFeatureEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    return useQuery({
        queryKey: ['iota-name', 'default-name', address, normalized],
        queryFn: async () => {
            if (!address) return null;

            const defaultName = await iotaNamesClient?.getDefaultName(address);

            if (!defaultName) return null;

            return normalized ? normalizeIotaName(defaultName) : defaultName;
        },
        enabled: !!iotaNamesClient && isFeatureEnabled && !!address,
        staleTime: 1000 * 60 * 10,
    });
}
