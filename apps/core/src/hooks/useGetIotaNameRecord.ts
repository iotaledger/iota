// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaNamesClient } from '../contexts';
import { useQuery } from '@tanstack/react-query';
import { isValidIotaName } from '@iota/iota-names-sdk';
import { shouldResolveInputAsName } from '../utils/validation/names';

export function useGetIotaNameRecord(value: string | null | undefined) {
    const { iotaNamesClient } = useIotaNamesClient();

    const isValid = isValidIotaName(value ?? '');
    const isNameInput = shouldResolveInputAsName(value ?? '');

    return useQuery({
        queryKey: ['iota-name', 'get-name-record', value, iotaNamesClient],
        queryFn: async () => {
            const nameRecord = await iotaNamesClient?.getNameRecord(value ?? '');

            if (!nameRecord) return null;
            return nameRecord;
        },
        enabled: !!iotaNamesClient && !!value && isValid && isNameInput,
        staleTime: 1000 * 60 * 10,
    });
}
