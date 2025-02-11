// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaObjectDataOptions, IotaObjectResponse } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery, UseQueryOptions } from '@tanstack/react-query';
import { chunkArray } from '../utils/chunkArray';

const defaultOptions = {
    showType: true,
    showContent: true,
    showOwner: true,
    showPreviousTransaction: true,
    showStorageRebate: true,
    showDisplay: true,
};

export function useMultiGetNormalizedObjects(
    ids: string[],
    options: IotaObjectDataOptions = defaultOptions,
    queryOptions?: Omit<UseQueryOptions<IotaObjectResponse[]>, 'queryKey' | 'queryFn'>,
) {
    const client = useIotaClient();

    const normalizedIds = ids.map((id) => normalizeIotaAddress(id));

    return useQuery({
        ...queryOptions,
        queryKey: ['multiGetObjects', normalizedIds],
        queryFn: async () => {
            const responses = await Promise.all(
                chunkArray(normalizedIds, 50).map((chunk) =>
                    client.multiGetObjects({
                        ids: chunk,
                        options,
                    }),
                ),
            );
            return responses.flat();
        },
        enabled: normalizedIds.length > 0,
    });
}
