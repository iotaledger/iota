// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { AssetCategory } from '@/lib/enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetTile } from '@/components';
import { useExplorerLinkGenerator } from '@/hooks';
import Link from 'next/link';

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
    const getExplorerLink = useExplorerLinkGenerator();

    function getAssetLinkProps(asset: IotaObjectData): React.ComponentProps<typeof Link> {
        if (selectedCategory === AssetCategory.Visual) {
            return { href: ASSETS_ROUTE.path + `/${asset.objectId}` };
        } else {
            return {
                href: getExplorerLink(asset.objectId),
                target: '_blank',
                rel: 'noopener noreferrer',
            };
        }
    }

    return (
        <div className={ASSET_LAYOUT[selectedCategory]}>
            {assets.map((asset) => (
                <Link key={asset.digest} {...getAssetLinkProps(asset)}>
                    <AssetTile asset={asset} type={selectedCategory} />
                </Link>
            ))}
        </div>
    );
}
