// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useGetStardustSharedBasicObjects } from './useGetStardustSharedBasicObjects';
import { useGetStardustSharedNftObjects } from './useGetStardustSharedNftObjects';
import { useEffect, useState } from 'react';

const LIMIT_PER_REQ = 50;

export function useGetAllStardustSharedObjects(address: string) {
    const [allBasicOutputs, setAllBasicOutputs] = useState<IotaObjectData[]>([]);
    const [allNftOutputs, setAllNftOutputs] = useState<IotaObjectData[]>([]);
    const [basicPage, setBasicPage] = useState(1);
    const [nftPage, setNftPage] = useState(1);

    const { data: basicObjects } = useGetStardustSharedBasicObjects(
        address,
        LIMIT_PER_REQ,
        basicPage,
    );

    const { data: nftObjects } = useGetStardustSharedNftObjects(address, LIMIT_PER_REQ, nftPage);

    useEffect(() => {
        console.log('basicObjects', basicObjects);
        if (basicObjects && basicObjects.length > 0) {
            setAllBasicOutputs((prev) => [
                ...prev,
                ...(basicObjects as unknown as IotaObjectData[]),
            ]);

            if (basicObjects.length === LIMIT_PER_REQ) {
                setBasicPage((prev) => prev + 1);
            }
        }
    }, [basicObjects]);

    useEffect(() => {
        console.log('nftObjects', nftObjects);
        if (nftObjects && nftObjects.length > 0) {
            setAllNftOutputs((prev) => [...prev, ...(nftObjects as unknown as IotaObjectData[])]);

            if (nftObjects.length === LIMIT_PER_REQ) {
                setNftPage((prev) => prev + 1);
            }
        }
    }, [nftObjects]);

    return useQuery({
        queryKey: ['stardust-all-shared-objects', address, allBasicOutputs, allNftOutputs],
        queryFn: async () => ({
            basic: allBasicOutputs,
            nfts: allNftOutputs,
        }),
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: { basic: [], nfts: [] },
    });
}
