// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useQuery } from '@tanstack/react-query';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useGetStardustSharedBasicObjects } from './useGetStardustSharedBasicObjects';
import { useGetStardustSharedNftObjects } from './useGetStardustSharedNftObjects';

const LIMIT_PER_REQ = 50;

export function useGetAllStardustSharedObjects(address: string) {
    const fetchPaginatedData = async () => {
        let allBasicOutputs: IotaObjectData[] = [];
        let allNftOutputs: IotaObjectData[] = [];

        let basicOutputPage = 1;
        do {
            const { data: basicObjects } = await useGetStardustSharedBasicObjects(
                address,
                LIMIT_PER_REQ,
                basicOutputPage,
            );

            if (!basicObjects || !basicObjects?.length) {
                break;
            }

            allBasicOutputs = [
                ...allBasicOutputs,
                ...(basicObjects as unknown as IotaObjectData[]),
            ];

            basicOutputPage = basicObjects.length < LIMIT_PER_REQ ? 0 : basicOutputPage + 1;
        } while (basicOutputPage > 0);

        let nftOutputPage = 1;
        do {
            const { data: nftObjects } = await useGetStardustSharedNftObjects(
                address,
                LIMIT_PER_REQ,
                nftOutputPage,
            );

            if (!nftObjects || !nftObjects?.length) {
                break;
            }

            allNftOutputs = [...allNftOutputs, ...(nftObjects as unknown as IotaObjectData[])];

            nftOutputPage = nftObjects.length < LIMIT_PER_REQ ? 0 : nftOutputPage + 1;
        } while (nftOutputPage > 0);

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
