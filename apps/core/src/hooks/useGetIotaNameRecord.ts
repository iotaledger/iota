// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';
import { useIotaNamesClient } from '../contexts';
import { useFeatureEnabledByNetwork } from './useFeatureEnabledByNetwork';
import { Feature } from '../enums';
import { useQuery } from '@tanstack/react-query';
import { isValidIotaName } from '@iota/iota-names-sdk';
import { shouldResolveInputAsName } from '../utils/validation/names';

export function useGetIotaNameRecord(value: string | null | undefined) {
    const networkName = useNetwork();
    const network = getNetwork(networkName).id;

    const { iotaNamesClient } = useIotaNamesClient();

    const isFeatureEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    const isValid = isValidIotaName(value ?? '');
    const isNameInput = shouldResolveInputAsName(value ?? '');

    return useQuery({
        queryKey: ['iota-name', 'get-name-record', value, iotaNamesClient, isFeatureEnabled],
        queryFn: async () => {
            const nameRecord = await iotaNamesClient?.getNameRecord(value ?? '');

            if (!nameRecord) return null;
            return nameRecord;
        },
        enabled: !!iotaNamesClient && isFeatureEnabled && !!value && isValid && isNameInput,
        staleTime: 1000 * 60 * 10,
    });
}
