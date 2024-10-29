// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AssetCard, PageSizeSelector, PaginationOptions } from '@/components';
import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { Panel, Title, Chip, TitleSize, DropdownPosition } from '@iota/apps-ui-kit';
import { hasDisplayData, useCursorPagination, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useMemo, useState } from 'react';
import { AssetCategory } from '@/lib/enums';
import { VisibilityOff } from '@iota/ui-icons';
import { useRouter } from 'next/navigation';

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
    // {
    //     label: 'Hidden',
    //     value: AssetCategory.Hidden,
    // },
];

export default function AssetsDashboardPage(): React.JSX.Element {
    const [selectedCategory, setSelectedCategory] = useState<AssetCategory>(AssetCategory.Visual);
    const [limit, setLimit] = useState<number>(PAGINATION_RANGE[1]);
    const router = useRouter();

    const account = useCurrentAccount();
    const ownedObjectsQuery = useGetOwnedObjects(account?.address, undefined, limit);

    const { data, pagination } = useCursorPagination(ownedObjectsQuery);

    const { data: ownedObjects } = data || {};

    const [visual, nonVisual] = useMemo(() => {
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
    }, [ownedObjects]);

    const categoryToAsset: Record<AssetCategory, IotaObjectData[]> = {
        [AssetCategory.Visual]: visual,
        [AssetCategory.Other]: nonVisual,
        // [AssetCategory.Hidden]: [],
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

                <div className="grid-template-visual-assets grid max-h-[600px] gap-md overflow-auto py-sm">
                    {assetList.map((asset) => (
                        <div key={asset.digest}>
                            <AssetCard
                                asset={asset}
                                icon={<VisibilityOff />}
                                onClick={() =>
                                    router.push(ASSETS_ROUTE.path + `/${asset.objectId}`)
                                }
                            />
                        </div>
                    ))}
                </div>
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
            </div>
        </Panel>
    );
}
