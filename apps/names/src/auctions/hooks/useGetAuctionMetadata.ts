// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { createAuctionMetadataQuery } from '../lib/utils/metadata';
import { useAuctionHouse } from './useAuctionHouse';

export function useGetAuctionMetadata(name: string) {
    const { iotaNamesClient } = useIotaNamesClient();
    const { data: auctionHouseData } = useAuctionHouse();

    const { auctionsTableObjectId } = auctionHouseData || {};
    const packageId = iotaNamesClient.getPackage('packageId', 'v1');

    const queryConfig = createAuctionMetadataQuery({
        name,
        auctionsTableObjectId,
        packageId,
        graphQLClient: iotaNamesClient.graphQlClient,
    });

    return useQuery(queryConfig);
}
