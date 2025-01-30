// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useStardustIndexerClientContext } from '../contexts';

export function useGetStardustSharedNftObjects(address: string, pageSize?: number, page?: number) {
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    return useQuery({
        queryKey: [
            'stardust-shared-nft-objects',
            address,
            pageSize,
            page,
            stardustIndexerClient?.baseUrl,
        ],
        queryFn: async () => {
            if (!stardustIndexerClient?.baseUrl) return [];

            const nftOutputs = await stardustIndexerClient.getNftResolvedOutputs(address, {
                page,
                pageSize,
            });

            return nftOutputs || [];
        },
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: [],
    });
}
