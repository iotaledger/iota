// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

const IOTA_NS_FEATURE_FLAG = 'iotans';

// This should align with whatever names we want to be able to resolve.
const IOTA_NS_DOMAINS = ['.iota'];
export function isIotaNSName(name: string) {
    return IOTA_NS_DOMAINS.some((domain) => name.endsWith(domain));
}

export function useIotaNSEnabled() {
    return useFeatureIsOn(IOTA_NS_FEATURE_FLAG);
}

export function useResolveIotaNSAddress(name?: string | null, enabled?: boolean) {
    const client = useIotaClient();
    const enabledIotaNs = useIotaNSEnabled();

    return useQuery({
        queryKey: ['resolve-iotans-address', name],
        queryFn: async () => {
            return await client.resolveNameServiceAddress({
                name: name!,
            });
        },
        enabled: !!name && enabled && enabledIotaNs,
        refetchOnWindowFocus: false,
        retry: false,
    });
}

export function useResolveIotaNSName(address?: string | null) {
    const client = useIotaClient();
    const enabled = useIotaNSEnabled();

    return useQuery({
        queryKey: ['resolve-iotans-name', address],
        queryFn: async () => {
            // NOTE: We only fetch 1 here because it's the default name.
            const { data } = await client.resolveNameServiceNames({
                address: address!,
                limit: 1,
            });

            return data[0] || null;
        },
        enabled: !!address && enabled,
        refetchOnWindowFocus: false,
        retry: false,
    });
}
