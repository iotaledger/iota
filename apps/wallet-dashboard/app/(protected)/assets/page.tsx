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
        isFetching,
        fetchNextPage,
        hasNextPage,
        refetch,
    } = useGetNFTs(account?.address, {
        MatchNone: [{ StructType: COIN_TYPE }],
    });

    const assets = (ownedAssets?.pages || [])
        .flatMap((page) => page.data)
        .filter((asset) => {
            if (!asset.data || !asset.data.objectId) {
                return false;
            }
            if (selectedCategory === AssetCategory.Visual) {
                return hasDisplayData({ data: asset });
            }
            if (selectedCategory === AssetCategory.Other) {
                return !hasDisplayData({ data: asset });
            }
            return false;
        })
        .map((asset) => asset.data)
        .filter((data): data is IotaObjectData => data !== null && data !== undefined);

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

    return (
        <Panel>
            <Title title="Assets" size={TitleSize.Medium} />
            <div className="px-lg">
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
                {selectedCategory && (
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
