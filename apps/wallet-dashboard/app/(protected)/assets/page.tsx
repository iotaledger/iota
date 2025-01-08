// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Panel, Title, Chip, TitleSize } from '@iota/apps-ui-kit';
import { COIN_TYPE, hasDisplayData, useGetNFTs } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useState, useEffect } from 'react';
import { AssetCategory } from '@/lib/enums';
import { AssetList } from '@/components/AssetsList';
import { AssetDialog } from '@/components/Dialogs/Assets';

const ASSET_CATEGORIES: { label: string; value: AssetCategory }[] = [
    {
        label: 'Visual',
        value: AssetCategory.Visual,
    },
    {
        label: 'Other',
        value: AssetCategory.Other,
    },
];

export default function AssetsDashboardPage(): React.JSX.Element {
    const [selectedAsset, setSelectedAsset] = useState<IotaObjectData | null>(null);
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory | null>(null);
    const account = useCurrentAccount();
    const {
        data: ownedAssets,
        hasNextPage,
        fetchNextPage,
        isPending,
        isFetching,
        refetch,
        isFetchingNextPage,
    } = useGetNFTs(account?.address, {
        MatchNone: [{ StructType: COIN_TYPE }],
    });
    const isAssetsLoaded = !!ownedAssets;

    let assets: IotaObjectData[] = [];

    if (selectedCategory === AssetCategory.Visual) {
        assets = ownedAssets?.visual || [];
    }

    if (selectedCategory === AssetCategory.Other) {
        assets =
            ownedAssets?.other
                .filter((asset) => {
                    return !hasDisplayData({ data: asset });
                })
                .filter((asset) => asset !== null && asset !== undefined) || [];
    }

    function onAssetClick(asset: IotaObjectData) {
        setSelectedAsset(asset);
    }

    useEffect(() => {
        if (!ownedAssets || selectedCategory !== null) {
            return;
        }

        const defaultCategory =
            ownedAssets.visual.length > 0
                ? AssetCategory.Visual
                : ownedAssets.other.length > 0
                  ? AssetCategory.Other
                  : AssetCategory.Visual;
        setSelectedCategory(defaultCategory);
    }, [ownedAssets, selectedCategory]);

    useEffect(() => {
        // Fetch the next page if there are no visual assets, other + hidden assets are present in multiples of 50, and there are more pages to fetch
        if (
            hasNextPage &&
            ownedAssets?.visual.length === 0 &&
            ownedAssets?.other.length + ownedAssets?.hidden.length > 0 &&
            (ownedAssets.other.length + ownedAssets.hidden.length) % 50 === 0 &&
            !isFetchingNextPage
        ) {
            fetchNextPage();
            setSelectedCategory(null);
        }
    }, [hasNextPage, ownedAssets, isFetchingNextPage, fetchNextPage]);

    return (
        <Panel>
            <Title title="Assets" size={TitleSize.Medium} />
            <div className="px-lg">
                {isAssetsLoaded && Boolean(assets) ? (
                    <div className="flex flex-row items-center justify-start gap-xs py-xs">
                        {ASSET_CATEGORIES.map((tab) => (
                            <Chip
                                key={tab.label}
                                label={tab.label}
                                onClick={() => setSelectedCategory(tab.value)}
                                selected={selectedCategory === tab.value}
                            />
                        ))}
                    </div>
                ) : null}
                {!isPending && selectedCategory && (
                    <AssetList
                        assets={assets}
                        selectedCategory={selectedCategory}
                        onClick={onAssetClick}
                        hasNextPage={hasNextPage}
                        isFetchingNextPage={isFetching}
                        fetchNextPage={fetchNextPage}
                    />
                )}
                {selectedAsset && (
                    <AssetDialog
                        onClose={() => setSelectedAsset(null)}
                        asset={selectedAsset}
                        refetchAssets={refetch}
                    />
                )}
            </div>
        </Panel>
    );
}
