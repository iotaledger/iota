// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { isKioskOwnerToken, useKioskClient } from '@iota/core';
import { AssetCategory } from '@/lib/enums';
import { VisibilityOff } from '@iota/ui-icons';
import { VisualAssetTile, KioskTile } from '.';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { NonVisualAssetCard } from './NonVisualAssetTile';

interface AssetTileLinkProps {
    asset: IotaObjectData;
    type: AssetCategory;
    onClick: (asset: IotaObjectData) => void;
}

export function AssetTileLink({ asset, type, onClick }: AssetTileLinkProps): React.JSX.Element {
    const kioskClient = useKioskClient();
    const isOwnerToken = isKioskOwnerToken(kioskClient.network, asset);
    function handleClick() {
        onClick(asset);
    }

    return (
        <>
            {type === AssetCategory.Visual && isOwnerToken ? (
                <KioskTile object={asset} />
            ) : type === AssetCategory.Visual ? (
                <VisualAssetTile asset={asset} icon={<VisibilityOff />} onClick={handleClick} />
            ) : (
                <NonVisualAssetCard asset={asset} />
            )}
        </>
    );
}
