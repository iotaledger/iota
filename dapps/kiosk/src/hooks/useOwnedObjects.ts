// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
/* eslint-disable @tanstack/query/exhaustive-deps */

import { useIotaClient } from '@iota/dapp-kit';
import { PaginatedObjectsResponse } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

import { TANSTACK_OWNED_OBJECTS_KEY } from '../utils/constants';
import { parseObjectDisplays } from '../utils/utils';

export function useOwnedObjects({
    address,
    cursor = undefined,
    limit = 50,
}: {
    address: string;
    cursor?: string;
    limit?: number;
}) {
    const provider = useIotaClient();

    return useQuery({
        queryKey: [TANSTACK_OWNED_OBJECTS_KEY, address],
        queryFn: async () => {
            if (!address) return [];
            const { data }: PaginatedObjectsResponse = await provider.getOwnedObjects({
                owner: address,
                options: {
                    showDisplay: true,
                    showType: true,
                },
                cursor,
                limit,
            });

            if (!data) return;

            const displays = parseObjectDisplays(data);

            // Simple mapping to OwnedObject style.
            return data.map((item) => ({
                display: displays[item.data?.objectId!] || {},
                type:
                    item.data?.type ??
                    (item?.data?.content?.dataType === 'package'
                        ? 'package'
                        : item?.data?.content?.type) ??
                    '',
                isLocked: false,
                objectId: item.data?.objectId,
            }));
        },
    });
}
