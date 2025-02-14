// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { mapStardustBasicOutputs } from '../utils';
import { useQuery } from '@tanstack/react-query';
import { useStardustIndexerClientContext } from '../contexts';

export function useGetStardustSharedBasicObjects(
    address: string,
    pageSize?: number,
    page?: number,
) {
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    return useQuery({
        queryKey: [
            'stardust-shared-basic-objects',
            address,
            pageSize,
            page,
            stardustIndexerClient?.baseUrl,
        ],
        queryFn: async () => {
            if (!stardustIndexerClient?.baseUrl) return [];

            const basicOutputs =
                (await stardustIndexerClient.getBasicResolvedOutputs(address, {
                    page,
                    pageSize,
                })) || [];
            return basicOutputs.map(mapStardustBasicOutputs) || [];
        },
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: [],
    });
}
