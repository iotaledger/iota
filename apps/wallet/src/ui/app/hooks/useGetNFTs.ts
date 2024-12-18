// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { hasDisplayData, isKioskOwnerToken, useGetOwnedObjects } from '@iota/core';
import { useKioskClient } from '@iota/core/src/hooks/useKioskClient';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { useMemo } from 'react';

import { useBuyNLargeAssets } from '../components/buynlarge/useBuyNLargeAssets';
import { useHiddenAssets } from '../pages/home/hidden-assets/HiddenAssetsProvider';

type OwnedAssets = {
    visual: IotaObjectData[];
    other: IotaObjectData[];
    hidden: IotaObjectData[];
};

export enum AssetFilterTypes {
    visual = 'visual',
    other = 'other',
}

export function useGetNFTs(address?: string | null) {
    const kioskClient = useKioskClient();
    const bnl = useBuyNLargeAssets();
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
            MatchNone: [
                { StructType: '0x2::coin::Coin' },
                ...(bnl
                    .filter((item) => !!item?.objectType)
                    .map((item) => ({ StructType: item?.objectType })) as { StructType: string }[]),
            ],
        },
        50,
    );
    const { hiddenAssetIds } = useHiddenAssets();

    const assets = useMemo(() => {
        const ownedAssets: OwnedAssets = {
            visual: [],
            other: [],
            hidden: [],
        };

        const groupedAssets = data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => !hiddenAssetIds.includes(asset.data?.objectId!))
            .reduce((acc, curr) => {
                if (hasDisplayData(curr) || isKioskOwnerToken(kioskClient.network, curr))
                    acc.visual.push(curr.data as IotaObjectData);
                if (!hasDisplayData(curr)) acc.other.push(curr.data as IotaObjectData);
                if (hiddenAssetIds.includes(curr.data?.objectId!))
                    acc.hidden.push(curr.data as IotaObjectData);
                return acc;
            }, ownedAssets);

        bnl.forEach((item) => {
            if (item?.asset?.data) {
                groupedAssets?.visual.unshift(item.asset.data);
            }
        });

        return groupedAssets;
    }, [hiddenAssetIds, data?.pages, kioskClient.network, bnl]);

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
