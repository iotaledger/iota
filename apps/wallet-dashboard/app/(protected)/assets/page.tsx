// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { PageSizeSelector, PaginationOptions } from '@/components';
import { Panel, Title, Chip, TitleSize, DropdownPosition } from '@iota/apps-ui-kit';
import { hasDisplayData, useCursorPagination, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { AssetCategory } from '@/lib/enums';
import { AssetList } from '@/components/AssetsList';
import { AssetsDialog, AssetsDialogView, useAssetsDialog } from '@/components/Dialogs/Assets';

const PAGINATION_RANGE = [20, 40, 60];

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
    const { view, setView } = useAssetsDialog();
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory>(AssetCategory.Visual);
    const [limit, setLimit] = useState<number>(PAGINATION_RANGE[1]);
    const [selectedAsset, setSelectedAsset] = useState<IotaObjectData | null>(null);

    const account = useCurrentAccount();
    const ownedObjectsQuery = useGetOwnedObjects(account?.address, undefined, limit);

    const { data, pagination } = useCursorPagination(ownedObjectsQuery);

    const { data: ownedObjects } = data || {};

    const [visual, nonVisual] = (() => {
        const visual: IotaObjectData[] = [];
        const nonVisual: IotaObjectData[] = [];

        ownedObjects
            ?.filter((asset) => asset.data && asset.data.objectId)
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

    function handleClickAsset(asset: IotaObjectData) {
        setSelectedAsset(asset);
        setView(AssetsDialogView.Details);
    }

    function handleCloseDialog() {
        setSelectedAsset(null);
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
                    assets={assetList}
                    selectedCategory={selectedCategory}
                    onClick={handleClickAsset}
                />
                <div className="flex flex-row items-center justify-end py-xs">
                    <PaginationOptions
                        pagination={pagination}
                        action={
                            <PageSizeSelector
                                pagination={pagination}
                                range={PAGINATION_RANGE}
                                dropdownPosition={DropdownPosition.Top}
                                setLimit={(e) => setLimit(e)}
                                limit={limit.toString()}
                            />
                        }
                    />
                </div>
                {view && (
                    <AssetsDialog
                        view={view}
                        setView={setView}
                        isOpen={!!selectedAsset}
                        handleClose={handleCloseDialog}
                        asset={selectedAsset}
                    />
                )}
            </div>
        </Panel>
    );
}
