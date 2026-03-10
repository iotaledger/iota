// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData, type IotaObjectDataFilter } from '@iota/iota-sdk/client';
import { useQuery, type UseQueryOptions } from '@tanstack/react-query';

import { queryKey } from './queryKey';

const MAX_OBJECTS_PER_REQ = 10;

export function useGetAllOwnedObjects<Data>(
    address: string,
    filter?: IotaObjectDataFilter,
    options?: Omit<UseQueryOptions<IotaObjectData[], Error, Data>, 'queryKey' | 'queryFn'>,
) {
    const client = useIotaClient();
    return useQuery({
        ...options,
        queryKey: [...queryKey.ownedObjects(address), filter],
        queryFn: async () => {
            let cursor: string | undefined | null = null;
            const allData: IotaObjectData[] = [];
            // keep fetching until cursor is null or undefined
            do {
                const { data: objectResponse, nextCursor } = await client.getOwnedObjects({
                    owner: address,
                    filter,
                    options: {
                        showType: true,
                        showContent: true,
                        showDisplay: true,
                    },
                    limit: MAX_OBJECTS_PER_REQ,
                    cursor,
                });
                if (!objectResponse || !objectResponse.length) {
                    break;
                }

                const mappedData = objectResponse
                    .map((response) => response?.data)
                    .filter((obj): obj is IotaObjectData => obj !== null && obj !== undefined);

                allData.push(...mappedData);
                cursor = nextCursor;
            } while (cursor);

            return allData;
        },
        staleTime: 10 * 1000,
        enabled: !!address,
    });
}
