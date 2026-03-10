// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { CoinMetadata } from '@iota/iota-sdk/client';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';
import { IOTA_DECIMALS, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '../contexts';

const ELLIPSIS = '\u{2026}';
const SYMBOL_TRUNCATE_LENGTH = 5;
const NAME_TRUNCATE_LENGTH = 10;

export function useCoinMetadata(coinType?: string | null) {
    const client = useIotaClient();
    const { iotaNamesClient } = useIotaNamesClient();

    return useQuery({
        queryKey: ['coin-metadata', coinType],
        queryFn: async () => {
            if (!coinType) {
                console.warn('coinType is null or undefined');
                return null;
            }

            if (coinType === IOTA_TYPE_ARG) {
                return IOTA_COIN_METADATA;
            }

            try {
                const rpcData = await client.getCoinMetadata({ coinType });

                if (rpcData) return rpcData;

                if (!iotaNamesClient) return null;

                // The RPC Node does not currently expose querying coin metadata of migrated coins,
                // but the GraphQL Node does

                const structType = `0x2::coin_manager::CoinManager<${coinType}>`;
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                const { data: graphqlData } = await iotaNamesClient.graphQlClient.query<any>({
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
                    graphqlData['objects']['nodes'][0]?.asMoveObject?.contents?.json?.metadata ??
                    undefined;

                if (coinMetadata) return coinMetadata;

                return null;
            } catch (err) {
                console.error('Failed to fetch coin metadata:', err);
                throw err;
            }
        },
        select(data) {
            if (!data) return null;

            return {
                ...data,
                symbol:
                    data.symbol.length > SYMBOL_TRUNCATE_LENGTH
                        ? data.symbol.slice(0, SYMBOL_TRUNCATE_LENGTH) + ELLIPSIS
                        : data.symbol,
                name:
                    data.name.length > NAME_TRUNCATE_LENGTH
                        ? data.name.slice(0, NAME_TRUNCATE_LENGTH) + ELLIPSIS
                        : data.name,
            };
        },
        retry: false,
        enabled: !!coinType,
        staleTime: Infinity,
        gcTime: 24 * 60 * 60 * 1000,
    });
}

export const IOTA_COIN_METADATA: CoinMetadata = {
    id: null,
    decimals: IOTA_DECIMALS,
    description: '',
    iconUrl: null,
    name: 'IOTA',
    symbol: 'IOTA',
};
