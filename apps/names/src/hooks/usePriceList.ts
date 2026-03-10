// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { queryKey } from './queryKey';

export function usePriceList() {
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: [...queryKey.priceList()],
        queryFn: async () => {
            const priceList = await iotaNamesClient.getPriceList();
            const nameLengthRanges = Array.from(priceList.keys()) as [number, number][];
            const minLength = Math.min(...nameLengthRanges.map(([from]) => from));
            const maxLength = Math.max(...nameLengthRanges.map(([, to]) => to));

            return { priceList, minLength, maxLength };
        },
        staleTime: 60 * 60 * 1000,
    });
}
