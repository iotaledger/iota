// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { IotaNamesClient, normalizeIotaName } from '@iota/iota-names-sdk';
import { useIotaClientContext } from './useIotaClient.js';
import { useMemo } from 'react';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';

export function useGetDefaultIotaName(
    address: string | null | undefined,
    iotaNamesEnabled: boolean,
) {
    const iotaContext = useIotaClientContext();
    const network = getNetwork(iotaContext.network);

    const iotaNamesClient = useMemo(() => {
        const iotaGraphQLClient = new IotaGraphQLClient({
            url: network.graphql!,
        });

        return new IotaNamesClient({
            graphQlClient: iotaGraphQLClient,
            network: network.id,
        });
    }, [network.id]);

    const iotaNamesSupported = !!iotaNamesClient.config;

    return useQuery({
        queryKey: ['iota-name', 'default-name', address],
        queryFn: async () => {
            if (!address) return null;

            const name = await iotaNamesClient?.getDefaultName(address);

            return name ? normalizeIotaName(name) : name;
        },
        enabled: !!iotaNamesClient && !!address && iotaNamesEnabled && iotaNamesSupported,
        staleTime: 1000 * 60 * 10,
    });
}
