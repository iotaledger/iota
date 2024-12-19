// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Panel, Title, Chip, TitleSize } from '@iota/apps-ui-kit';
import { hasDisplayData, useGetNFTs } from '@iota/core';
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
    } = useGetNFTs(account?.address);

    const assets: IotaObjectData[] = (() => {
        if (selectedCategory === null) return [] as IotaObjectData[];

        const assetsList = ownedAssets ? ownedAssets[selectedCategory] : [];
        return assetsList.filter((asset) => {
            if (selectedCategory === AssetCategory.Visual) {
                return hasDisplayData({ data: asset });
            }
            return true;
        });
    })();

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
