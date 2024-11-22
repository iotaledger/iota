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
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory>(AssetCategory.Visual);
    const account = useCurrentAccount();
    const { data, isFetching, fetchNextPage, hasNextPage } = useGetOwnedObjects(
        account?.address,
        undefined,
        50,
    );

    const [visual, nonVisual] = (() => {
        const visual: IotaObjectData[] = [];
        const nonVisual: IotaObjectData[] = [];

        data?.pages.forEach((page) =>
            page.data.forEach((asset) => {
                if (asset.data && asset.data.objectId) {
                    if (hasDisplayData(asset)) {
                        visual.push(asset.data);
                    } else {
                        nonVisual.push(asset.data);
                    }
                }
            }),
        );

        return [visual, nonVisual];
    })();

    const categoryToAsset: Record<AssetCategory, IotaObjectData[]> = {
        [AssetCategory.Visual]: visual,
        [AssetCategory.Other]: nonVisual,
    };

    const assetList = categoryToAsset[selectedCategory];

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
                    assets={assetList}
                    selectedCategory={selectedCategory}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetching}
                    fetchNextPage={fetchNextPage}
                />
            </div>
        </Panel>
    );
}
