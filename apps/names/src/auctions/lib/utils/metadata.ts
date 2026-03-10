// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNameType } from '@iota/iota-names-sdk';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';
import { fromBase64 } from '@iota/iota-sdk/utils';

import { queryKey } from '@/hooks';

import { AUCTION_MIN_OVERBID_VALUE_IOTA } from '../constants';
import { AuctionFieldBcs } from '../types/bcs';
import { AuctionMetadata, RawAuctionMetadata } from '../types/metadata';
import { deriveAuctionDynamicFieldId } from './dynamicField';
import { createNameObject } from './name';

interface FetchAuctionMetadataParams {
    name: string;
    auctionsTableObjectId: string;
    packageId: string;
    graphQLClient: IotaGraphQLClient;
}

export async function fetchAuctionMetadata({
    name,
    auctionsTableObjectId,
    packageId,
    graphQLClient,
}: FetchAuctionMetadataParams) {
    if (!auctionsTableObjectId || !name) {
        return null;
    }

    const nameObject = createNameObject(name);
    const nameTypeTag = getNameType(packageId);

    const targetAuctionObjectId = deriveAuctionDynamicFieldId(
        auctionsTableObjectId,
        nameTypeTag,
        nameObject,
    );

    const auctionObjectResponse = await graphQLClient.query<{
        object: {
            address: string;
            asMoveObject: {
                contents: {
                    bcs: string;
                };
            };
        };
    }>({
        query: graphql(`
            query getAuction($id: String!) {
                object(address: $id) {
                    address
                    asMoveObject {
                        contents {
                            bcs
                        }
                    }
                }
            }
        `),
        variables: {
            id: targetAuctionObjectId,
        },
    });

    return auctionObjectResponse.data;
}

/**
 * Converts raw BCS auction data to normalized TypeScript interface
 */
function normalizeAuctionData(
    rawData: RawAuctionMetadata,
    isActive: boolean,
): AuctionMetadata | null {
    if (!rawData?.value?.value) {
        return null;
    }

    const auction = rawData.value.value;

    const currentBidNanos = BigInt(auction.current_bid.balance.value || BigInt(0));
    const minBidNanos = currentBidNanos + AUCTION_MIN_OVERBID_VALUE_IOTA;

    return {
        name: auction.nft.name_str,
        startTimestamp: new Date(Number(auction.start_timestamp_ms)),
        endTimestamp: new Date(Number(auction.end_timestamp_ms)),
        winner: auction.winner,
        currentBidNanos,
        minBidNanos,
        nftId: auction.nft.id,
        nftExpiration: new Date(Number(auction.nft.expiration_timestamp_ms)),
        isActive,
    };
}

/**
 * Parses raw auction data into the bcs type.
 */
function parseAuctionDataBcs(
    data: Awaited<ReturnType<typeof fetchAuctionMetadata>>,
): ReturnType<typeof AuctionFieldBcs.parse> | null {
    const auctionBcsB64 = data?.object?.asMoveObject?.contents?.bcs;

    if (!auctionBcsB64) {
        return null;
    }

    try {
        return AuctionFieldBcs.parse(fromBase64(auctionBcsB64));
    } catch (error) {
        return null;
    }
}

interface CreateAuctionMetadataQueryParams {
    name: string;
    auctionsTableObjectId: string | undefined;
    packageId: string;
    graphQLClient: IotaGraphQLClient;
}

/**
 * Creates a React Query configuration object for fetching auction metadata
 */
export function createAuctionMetadataQuery({
    name,
    auctionsTableObjectId,
    packageId,
    graphQLClient,
}: CreateAuctionMetadataQueryParams) {
    return {
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [...queryKey.auctionMetadata(name), packageId, auctionsTableObjectId],
        queryFn: async () => {
            if (!auctionsTableObjectId) {
                return null;
            }

            const data = await fetchAuctionMetadata({
                name,
                auctionsTableObjectId,
                packageId,
                graphQLClient,
            });
            const auctionMetadata = parseAuctionDataBcs(data);
            const auction = auctionMetadata?.value.value;

            const now = new Date();
            const endTimestamp = new Date(Number(auction?.end_timestamp_ms));
            const isActive = now < endTimestamp;

            return { auctionMetadata, isActive };
        },
        select: (
            data: { auctionMetadata: RawAuctionMetadata | null; isActive: boolean } | null,
        ): AuctionMetadata | null => {
            if (!data?.auctionMetadata?.value?.value) return null;
            return normalizeAuctionData(data.auctionMetadata, data.isActive || false);
        },
        enabled: !!auctionsTableObjectId && !!name,
        refetchInterval: 10000,
    };
}
