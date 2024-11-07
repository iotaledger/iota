// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//[object Object]
//[object Object]
'use client';
// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { AssetCategory } from '@/lib/enums';
import { VisibilityOff } from '@iota/ui-icons';
import { VisualAssetTile } from '.';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { NonVisualAssetCard } from './NonVisualAssetTile';
import { useExplorerLinkGetter } from '@/hooks';
import Link from 'next/link';
import { ExplorerLinkType } from '@iota/core';

interface AssetTileLinkProps {
    asset: IotaObjectData;
    type: AssetCategory;
}

export function AssetTileLink({ asset, type }: AssetTileLinkProps): React.JSX.Element {
    const getExplorerLink = useExplorerLinkGetter();
    const linkProps = getAssetLinkProps(asset);

    function getAssetLinkProps(asset: IotaObjectData): React.ComponentProps<typeof Link> {
        if (type === AssetCategory.Visual) {
            return { href: ASSETS_ROUTE.path + `/${asset.objectId}` };
        } else {
            const explorerLink =
                getExplorerLink({
                    type: ExplorerLinkType.Object,
                    objectID: asset.objectId,
                }) ?? '';

            return {
                href: explorerLink,
                target: '_blank',
                rel: 'noopener noreferrer',
            };
        }
    }

    return (
        <Link {...linkProps}>
            {type === AssetCategory.Visual ? (
                <VisualAssetTile asset={asset} icon={<VisibilityOff />} />
            ) : (
                <NonVisualAssetCard asset={asset} />
            )}
        </Link>
    );
}
