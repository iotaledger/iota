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
import { AssetsDialog, AssetsDialogView } from '@/components/Dialogs/Assets';

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
    const [view, setView] = useState<AssetsDialogView>({ type: 'close', asset: undefined });
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory>(AssetCategory.Visual);
    const account = useCurrentAccount();
    const { data, isFetching, fetchNextPage, hasNextPage } = useGetOwnedObjects(
        account?.address,
        undefined,
        OBJECTS_PER_REQ,
    );

    const assets: IotaObjectData[] = [];

    for (const page of data?.pages || []) {
        for (const asset of page.data) {
            if (asset.data && asset.data.objectId) {
                if (selectedCategory == AssetCategory.Visual) {
                    if (hasDisplayData(asset)) {
                        assets.push(asset.data);
                    }
                } else if (selectedCategory == AssetCategory.Other) {
                    assets.push(asset.data);
                }
            }
        }
    }

    function onClickAsset(asset: IotaObjectData) {
        setView({
            type: 'details',
            asset,
        });
    }

    function onCloseDialog() {
        setView({ type: 'close', asset: undefined });
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
                    onClick={onClickAsset}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetching}
                    fetchNextPage={fetchNextPage}
                />
                <AssetsDialog view={view} setView={setView} onClose={onCloseDialog} />
            </div>
        </Panel>
    );
}
