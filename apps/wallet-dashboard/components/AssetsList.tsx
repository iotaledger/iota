// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { AssetCategory } from '@/lib/enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useRouter } from 'next/navigation';
import { AssetCard } from './AssetCard';

interface AssetListProps {
    assets: IotaObjectData[];
    selectedCategory: AssetCategory;
}

const ASSET_LAYOUT: Record<AssetCategory, string> = {
    [AssetCategory.Visual]:
        'grid-template-visual-assets grid max-h-[600px] gap-md overflow-auto py-sm',
    [AssetCategory.Other]: 'flex flex-col overflow-auto py-sm',
};

export function AssetList({ assets, selectedCategory }: AssetListProps): React.JSX.Element {
    const router = useRouter();

    function handleAssetClick(asset: IotaObjectData) {
        router.push(ASSETS_ROUTE.path + `/${asset.objectId}`);
    }

    return (
        <div className={ASSET_LAYOUT[selectedCategory]}>
            {assets.map((asset) => (
                <AssetCard
                    key={asset.digest}
                    asset={asset}
                    type={selectedCategory}
                    onClick={() => handleAssetClick(asset)}
                />
            ))}
        </div>
    );
}
