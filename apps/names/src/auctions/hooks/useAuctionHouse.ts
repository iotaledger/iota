// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';
import { fromBase64 } from '@iota/iota-sdk/utils';
import { useQuery, UseQueryResult } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { AuctionHouseBcs } from '../lib/types/bcs';

interface AuctionHouseData {
    auctionHouseId: string;
    auctionsTableObjectId: string;
    headAuction?: string;
    tailAuction?: string;
}

export function useAuctionHouse(): UseQueryResult<AuctionHouseData | null, Error> {
    const { iotaNamesClient } = useIotaNamesClient();

    const auctionPackageId = iotaNamesClient.getPackage('auctionPackageId', 'v1');

    return useQuery({
        queryKey: ['auctionHouse', auctionPackageId],
        async queryFn() {
            const auctionHouseByTypeResponse = await iotaNamesClient.graphQlClient.query<{
                objects: {
                    edges: [
                        { node: { address: string; asMoveObject: { contents: { bcs: string } } } },
                    ];
                };
            }>({
                query: graphql(`
                    query getActionHouse($type: String!) {
                        objects(filter: { type: $type }) {
                            edges {
                                node {
                                    address
                                    asMoveObject {
                                        contents {
                                            bcs
                                        }
                                    }
                                }
                            }
                        }
                    }
                `),
                variables: {
                    type: `${auctionPackageId}::auction::AuctionHouse`,
                },
            });

            return auctionHouseByTypeResponse.data;
        },
        select: (data) => {
            const auctionHouseData = data?.objects.edges[0]?.node ?? null;
            const auctionHouseBcsB64 = auctionHouseData?.asMoveObject?.contents?.bcs;

            if (!auctionHouseData?.address || !auctionHouseBcsB64) {
                return null;
            }

            const auctionHouse = AuctionHouseBcs.parse(fromBase64(auctionHouseBcsB64));

            return {
                auctionHouseId: auctionHouseData.address,
                auctionsTableObjectId: auctionHouse.auctions.id,
                headAuction: auctionHouse.auctions.head?.labels.reverse().join('.'),
                tailAuction: auctionHouse.auctions.tail?.labels.reverse().join('.'),
            };
        },
        enabled: !!auctionPackageId,
    });
}
