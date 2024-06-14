// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { hasDisplayData, isKioskOwnerToken, useGetOwnedObjects, useKioskClient } from '..';
import { type IotaObjectData } from '@iota/iota.js/client';
import { useMemo } from 'react';

type OwnedAssets = {
    visual: IotaObjectData[];
    other: IotaObjectData[];
};

export enum AssetFilterTypes {
    visual = 'visual',
    other = 'other',
}

export function useGetNFTs(address?: string | null) {
    const kioskClient = useKioskClient();
    const {
        data,
        isPending,
        error,
        isError,
        isFetchingNextPage,
        hasNextPage,
        fetchNextPage,
        isLoading,
    } = useGetOwnedObjects(
        address,
        {
            MatchNone: [{ StructType: '0x2::coin::Coin' }],
        },
        50,
    );

    const assets = useMemo(() => {
        const ownedAssets: OwnedAssets = {
            visual: [],
            other: [],
        };

        const groupedAssets = data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data?.objectId)
            .reduce((acc, curr) => {
                if (hasDisplayData(curr) || isKioskOwnerToken(kioskClient.network, curr))
                    acc.visual.push(curr.data as IotaObjectData);
                if (!hasDisplayData(curr)) acc.other.push(curr.data as IotaObjectData);
                return acc;
            }, ownedAssets);

        return groupedAssets;
    }, [data?.pages, kioskClient.network]);

    return {
        data: assets,
        isLoading,
        hasNextPage,
        isFetchingNextPage,
        fetchNextPage,
        isPending: isPending,
        isError: isError,
        error,
    };
}
