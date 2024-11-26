// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AssetCategory } from '@/lib/enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetTileLink } from '@/components';

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
    return (
        <div className={ASSET_LAYOUT[selectedCategory]}>
            {assets.map((asset) => (
                <AssetTileLink key={asset.digest} asset={asset} type={selectedCategory} />
            ))}
        </div>
    );
}
