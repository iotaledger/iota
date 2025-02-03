// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useQuery } from '@tanstack/react-query';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useGetStardustSharedBasicObjects } from './useGetStardustSharedBasicObjects';
import { useGetStardustSharedNftObjects } from './useGetStardustSharedNftObjects';
import { useState, useEffect, useMemo } from 'react';

const LIMIT_PER_REQ = 50;

export function useGetAllStardustSharedObjects(address: string) {
    // Use the unique key to reset state on every hook call
    const uniqueKey = useMemo(() => Math.random().toString(), [address]);

    const [basicOutputPage, setBasicOutputPage] = useState(1);
    const [nftOutputPage, setNftOutputPage] = useState(1);
    const [allBasicOutputs, setAllBasicOutputs] = useState<IotaObjectData[]>([]);
    const [allNftOutputs, setAllNftOutputs] = useState<IotaObjectData[]>([]);

    const basicObjects = useGetStardustSharedBasicObjects(address, LIMIT_PER_REQ, basicOutputPage);
    const nftObjects = useGetStardustSharedNftObjects(address, LIMIT_PER_REQ, nftOutputPage);

    useEffect(() => {
        setBasicOutputPage(1);
        setNftOutputPage(1);
        setAllBasicOutputs([]);
        setAllNftOutputs([]);
    }, [uniqueKey]);

    useEffect(() => {
        if (basicObjects.data && basicObjects.data.length > 0) {
            setAllBasicOutputs((prev) => [
                ...prev,
                ...(basicObjects.data as unknown as IotaObjectData[]),
            ]);

            if (basicObjects.data.length === LIMIT_PER_REQ) {
                setBasicOutputPage((prev) => prev + 1);
            }
        }
    }, [basicObjects.data, uniqueKey]);

    useEffect(() => {
        if (nftObjects.data && nftObjects.data.length > 0) {
            setAllNftOutputs((prev) => [
                ...prev,
                ...(nftObjects.data as unknown as IotaObjectData[]),
            ]);

            if (nftObjects.data.length === LIMIT_PER_REQ) {
                setNftOutputPage((prev) => prev + 1);
            }
        }
    }, [nftObjects.data, uniqueKey]);

    return useQuery({
        queryKey: [
            'stardust-all-shared-objects',
            address,
            uniqueKey,
            allBasicOutputs,
            allNftOutputs,
        ],
        queryFn: async () => ({
            basic: allBasicOutputs,
            nfts: allNftOutputs,
        }),
        enabled: !!address,
        staleTime: 1000 * 60 * 5,
        placeholderData: { basic: [], nfts: [] },
    });
}
