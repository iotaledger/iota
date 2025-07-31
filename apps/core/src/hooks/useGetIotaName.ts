// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useIotaNamesClientContext } from '../contexts';

export function useGetIotaName(address: string) {
    const { iotaNamesClient } = useIotaNamesClientContext();

    return useQuery({
        queryKey: ['iota-name', 'default-name', address],
        async queryFn() {
            const defaultName = await iotaNamesClient?.getDefaultName(address);

            return defaultName;
        },
        enabled: !!iotaNamesClient && address.length > 0,
    });
}
