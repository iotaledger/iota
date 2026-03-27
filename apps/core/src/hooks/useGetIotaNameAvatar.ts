// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';
import { useIotaNamesClient } from '../contexts';
import { useFeatureEnabledByNetwork } from './useFeatureEnabledByNetwork';
import { Feature } from '../enums';
import { useQuery } from '@tanstack/react-query';
import { useIotaClient } from '@iota/dapp-kit';

export function useGetIotaNameAvatar(address: string | null | undefined) {
    const networkName = useNetwork();
    const network = getNetwork(networkName).id;

    const { iotaNamesClient } = useIotaNamesClient();
    const isFeatureEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);
    const client = useIotaClient();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['iota-name', 'avatar', address, network],
        queryFn: async () => {
            if (!address || !iotaNamesClient) return null;

            const defaultName = await iotaNamesClient.getPublicName(address);
            if (!defaultName) return null;

            const nameRecord = await iotaNamesClient.getNameRecord(defaultName);
            if (!nameRecord) return null;

            const objectId = nameRecord.avatar ?? nameRecord.nftId;
            if (!objectId) return null;

            const objectResponse = await client.getObject({
                id: objectId,
                options: { showDisplay: true },
            });

            return objectResponse?.data?.display?.data?.image_url ?? null;
        },
        enabled: !!iotaNamesClient && isFeatureEnabled && !!address,
        staleTime: 1000 * 60 * 5,
    });
}
