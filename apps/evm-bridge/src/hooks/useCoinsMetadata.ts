// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_COIN_METADATA, useIotaGraphQLClientContext } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { CoinMetadata } from '@iota/iota-sdk/client';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQueries } from '@tanstack/react-query';
import { useMemo } from 'react';

const ELLIPSIS = '\u{2026}';
const SYMBOL_TRUNCATE_LENGTH = 5;
const NAME_TRUNCATE_LENGTH = 10;

/**
 * Fetches metadata for multiple coins efficiently
 * @param coins Array of coin balance objects that contain coinType
 * @returns A map of coinTypes to their metadata and loading states
 */
export function useCoinsMetadata(coins: Array<{ coinType: string }>) {
    const client = useIotaClient();
    const { iotaGraphQLClient } = useIotaGraphQLClientContext();

    // Filter out any duplicates to avoid redundant queries
    const uniqueCoinTypes = useMemo(
        () => [...new Set(coins.map((coin) => coin.coinType))],
        [coins],
    );

    // Use the same query pattern as useCoinMetadata but for multiple coins
    const queriesResults = useQueries({
        queries: uniqueCoinTypes.map((coinType) => ({
            queryKey: ['coin-metadata', coinType],
            queryFn: async () => {
                if (coinType === IOTA_TYPE_ARG) {
                    return IOTA_COIN_METADATA;
                }

                try {
                    const rpcData = await client.getCoinMetadata({ coinType });
                    if (rpcData) return rpcData;

                    // If RPC fails, try GraphQL as fallback
                    if (!iotaGraphQLClient) return null;

                    // The RPC Node does not currently expose querying coin metadata of migrated coins,
                    // but the GraphQL Node does
                    const structType = `0x2::coin_manager::CoinManager<${coinType}>`;

                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    const { data: graphqlData } = await iotaGraphQLClient.query<any>({
                        query: graphql(`
                            query getCoinManager($type: String!) {
                                objects(filter: { type: $type }) {
                                    nodes {
                                        asMoveObject {
                                            contents {
                                                json
                                            }
                                        }
                                    }
                                }
                            }
                        `),
                        variables: {
                            type: structType,
                        },
                    });

                    if (!graphqlData) return null;

                    const coinMetadata: CoinMetadata | undefined =
                        graphqlData['objects']['nodes'][0]?.asMoveObject?.contents?.json
                            ?.metadata ?? undefined;

                    if (coinMetadata) return coinMetadata;

                    return null;
                } catch (err) {
                    console.error('Failed to fetch coin metadata:', err);
                    throw err;
                }
            },
            select: (data: unknown) => {
                if (!data) return null;
                const coinData = data as CoinMetadata;
                // Same post-processing as in useCoinMetadata
                return {
                    ...coinData,
                    symbol:
                        coinData.symbol.length > SYMBOL_TRUNCATE_LENGTH
                            ? coinData.symbol.slice(0, SYMBOL_TRUNCATE_LENGTH) + ELLIPSIS
                            : coinData.symbol,
                    name:
                        coinData.name.length > NAME_TRUNCATE_LENGTH
                            ? coinData.name.slice(0, NAME_TRUNCATE_LENGTH) + ELLIPSIS
                            : coinData.name,
                };
            },
            retry: false,
            enabled: !!coinType,
            staleTime: Infinity,
            gcTime: 24 * 60 * 60 * 1000,
        })),
    });

    return useMemo(() => {
        const metadata: Record<string, CoinMetadata | null> = {};
        const isLoading: Record<string, boolean> = {};
        uniqueCoinTypes.forEach((coinType, index) => {
            const query = queriesResults[index];
            metadata[coinType] = query.data || null;
            isLoading[coinType] = query.isLoading;
        });
        return {
            metadata,
            isLoading,
        };
    }, [uniqueCoinTypes]);
}
