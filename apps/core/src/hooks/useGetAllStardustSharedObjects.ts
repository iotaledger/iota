// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useQuery } from '@tanstack/react-query';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useGetStardustSharedBasicObjects } from './useGetStardustSharedBasicObjects';
import { useGetStardustSharedNftObjects } from './useGetStardustSharedNftObjects';
import { useState, useEffect } from 'react';

const LIMIT_PER_REQ = 50;

export function useGetAllStardustSharedObjects(address: string) {
    const [basicOutputPage, setBasicOutputPage] = useState(1);
    const [nftOutputPage, setNftOutputPage] = useState(1);
    const [allBasicOutputs, setAllBasicOutputs] = useState<IotaObjectData[]>([]);
    const [allNftOutputs, setAllNftOutputs] = useState<IotaObjectData[]>([]);
    const [isBasicOutputComplete, setIsBasicOutputComplete] = useState(false);
    const [isNftOutputComplete, setIsNftOutputComplete] = useState(false);

    // Reset state when address changes
    useEffect(() => {
        setBasicOutputPage(1);
        setNftOutputPage(1);
        setAllBasicOutputs([]);
        setAllNftOutputs([]);
        setIsBasicOutputComplete(false);
        setIsNftOutputComplete(false);
    }, [address]);

    // Call hooks at the top level
    const basicObjects = useGetStardustSharedBasicObjects(address, LIMIT_PER_REQ, basicOutputPage);

    const nftObjects = useGetStardustSharedNftObjects(address, LIMIT_PER_REQ, nftOutputPage);

    // Handle basic objects pagination
    useEffect(() => {
        if (basicObjects.data && basicObjects.data.length > 0) {
            setAllBasicOutputs((prev) => [
                ...prev,
                ...(basicObjects.data as unknown as IotaObjectData[]),
            ]);

            if (basicObjects.data.length < LIMIT_PER_REQ) {
                setIsBasicOutputComplete(true);
            } else {
                setBasicOutputPage((prev) => prev + 1);
            }
        } else if (basicObjects.data?.length === 0) {
            setIsBasicOutputComplete(true);
        }
    }, [basicObjects.data]);

    // Handle NFT objects pagination
    useEffect(() => {
        if (nftObjects.data && nftObjects.data.length > 0) {
            setAllNftOutputs((prev) => [
                ...prev,
                ...(nftObjects.data as unknown as IotaObjectData[]),
            ]);

            if (nftObjects.data.length < LIMIT_PER_REQ) {
                setIsNftOutputComplete(true);
            } else {
                setNftOutputPage((prev) => prev + 1);
            }
        } else if (nftObjects.data?.length === 0) {
            setIsNftOutputComplete(true);
        }
    }, [nftObjects.data]);

    // Wrap the results in useQuery for consistency with your original API
    return useQuery({
        queryKey: [
            'stardust-all-shared-objects',
            address,
            basicOutputPage,
            nftOutputPage,
            allBasicOutputs,
            allNftOutputs,
        ],
        queryFn: async () => ({
            basic: allBasicOutputs,
            nfts: allNftOutputs,
        }),
        enabled: !!address && isBasicOutputComplete && isNftOutputComplete,
        staleTime: 1000 * 60 * 5,
        placeholderData: { basic: [], nfts: [] },
    });
}
