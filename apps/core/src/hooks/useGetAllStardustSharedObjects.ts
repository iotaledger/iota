// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useQuery } from '@tanstack/react-query';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useGetStardustSharedObjects } from './useGetStardustSharedObjects';

const LIMIT_PER_REQ = 50;

export function useGetAllStardustSharedObjects(address: string) {
    const fetchPaginatedData = async () => {
        let allBasicOutputs: IotaObjectData[] = [];
        let allNftOutputs: IotaObjectData[] = [];
        let page = 1;
        let hasMoreData = true;

        while (hasMoreData) {
            const { data } = await useGetStardustSharedObjects(address, LIMIT_PER_REQ, page);

            if (!data) break;

            allBasicOutputs = [...allBasicOutputs, ...(data.basic as unknown as IotaObjectData[])];
            allNftOutputs = [...allNftOutputs, ...(data.nfts as unknown as IotaObjectData[])];

            if (data.basic.length < LIMIT_PER_REQ && data.nfts.length < LIMIT_PER_REQ) {
                hasMoreData = false;
            } else {
                page++;
            }
        }

        return {
            basic: allBasicOutputs,
            nfts: allNftOutputs,
        };
    };

    return useQuery({
        queryKey: ['stardust-all-shared-objects', address],
        queryFn: fetchPaginatedData,
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: { basic: [], nfts: [] },
    });
}
