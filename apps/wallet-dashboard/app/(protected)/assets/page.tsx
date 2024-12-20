// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Panel, Title, Chip, TitleSize } from '@iota/apps-ui-kit';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { AssetCategory } from '@/lib/enums';
import { AssetList } from '@/components/AssetsList';
import { AssetDialog } from '@/components/Dialogs/Assets';

const OBJECTS_PER_REQ = 50;

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
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory>(AssetCategory.Visual);
    const account = useCurrentAccount();
    const { data, isFetching, fetchNextPage, hasNextPage, refetch } = useGetOwnedObjects(
        account?.address,
        {
            MatchNone: [{ StructType: '0x2::coin::Coin' }],
        },
        OBJECTS_PER_REQ,
    );

    const assets = new Map<string, IotaObjectData[]>(
        ASSET_CATEGORIES.map((tab) => [tab.value, []]),
    );

    for (const page of data?.pages || []) {
        for (const asset of page.data) {
            if (asset.data && asset.data.objectId) {
                if (hasDisplayData(asset)) {
                    assets.get(AssetCategory.Visual)?.push(asset.data);
                } else {
                    assets.get(AssetCategory.Other)?.push(asset.data);
                }
            }
        }
    }

    function onAssetClick(asset: IotaObjectData) {
        setSelectedAsset(asset);
    }

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

                <AssetList
                    assets={assets.get(selectedCategory) ?? []}
                    selectedCategory={selectedCategory}
                    onClick={onAssetClick}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetching}
                    fetchNextPage={fetchNextPage}
                />
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
