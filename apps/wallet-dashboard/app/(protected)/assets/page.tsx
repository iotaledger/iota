// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AssetCard, VirtualList } from '@/components';
import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { Panel, Title, Chip } from '@iota/apps-ui-kit';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { AssetCategory } from '@/lib/enums';
import Link from 'next/link';

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
    const {
        data: ownedObjects,
        fetchNextPage,
        hasNextPage,
        isFetchingNextPage,
    } = useGetOwnedObjects(account?.address);

    const [visual, nonVisual] = (() => {
        const visual: IotaObjectData[] = [];
        const nonVisual: IotaObjectData[] = [];

        ownedObjects?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId)
            .forEach((asset) => {
                if (asset.data) {
                    if (hasDisplayData(asset)) {
                        visual.push(asset.data);
                    } else {
                        nonVisual.push(asset.data);
                    }
                }
            });

        return [visual, nonVisual];
    })();

    const categoryToAsset: Record<AssetCategory, IotaObjectData[]> = {
        [AssetCategory.Visual]: visual,
        [AssetCategory.Other]: nonVisual,
    };

    const assetList = categoryToAsset[selectedCategory];

    return (
        <Panel>
            <Title title="Assets" />
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

                <div className="max-h-[600px] overflow-auto py-sm">
                    <VirtualList
                        items={assetList}
                        hasNextPage={hasNextPage}
                        isFetchingNextPage={isFetchingNextPage}
                        fetchNextPage={fetchNextPage}
                        estimateSize={() => 180}
                        render={(asset) => (
                            <Link href={ASSETS_ROUTE.path + `/${asset.objectId}`}>
                                <AssetCard asset={asset} />
                            </Link>
                        )}
                    />
                </div>
            </div>
        </Panel>
    );
}
