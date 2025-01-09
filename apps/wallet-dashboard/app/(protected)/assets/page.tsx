// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Panel, Title, Chip, TitleSize } from '@iota/apps-ui-kit';
import { COIN_TYPE, hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { AssetCategory } from '@/lib/enums';
import { AssetList } from '@/components/AssetsList';
import { AssetDialog } from '@/components/dialogs/assets';

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
            MatchNone: [{ StructType: COIN_TYPE }],
        },
        OBJECTS_PER_REQ,
    );

    const assets = (data?.pages || [])
        .flatMap((page) => page.data)
        .filter((asset) => {
            if (!asset.data || !asset.data.objectId) {
                return false;
            }
            if (selectedCategory === AssetCategory.Visual) {
                return hasDisplayData(asset);
            }
            if (selectedCategory === AssetCategory.Other) {
                return !hasDisplayData(asset);
            }
            return false;
        })
        .map((asset) => asset.data)
        .filter((data): data is IotaObjectData => data !== null && data !== undefined);

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
                    assets={assets}
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
