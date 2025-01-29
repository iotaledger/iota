// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useStardustIndexerClientContext } from '../contexts';

export function useGetStardustSharedObjects(address: string, pageSize?: number, page?: number) {
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    return useQuery({
        queryKey: ['stardust-shared-objects', address, pageSize, page, stardustIndexerClient],
        queryFn: async () => {
            if (!stardustIndexerClient) return { basic: [], nfts: [] };

            const [basicOutputs, nftOutputs] = await Promise.all([
                stardustIndexerClient.getBasicResolvedOutputs(address, { page, pageSize }),
                stardustIndexerClient.getNftResolvedOutputs(address, { page, pageSize }),
            ]);

            return {
                basic: basicOutputs || [],
                nfts: nftOutputs || [],
            };
        },
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: { basic: [], nfts: [] },
    });
}
