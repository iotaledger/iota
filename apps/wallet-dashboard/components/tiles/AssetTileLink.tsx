// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AssetCategory } from '@iota/core';
import { VisibilityOff } from '@iota/ui-icons';
import { VisualAssetTile } from '.';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { NonVisualAssetCard } from './NonVisualAssetTile';

interface AssetTileLinkProps {
    asset: IotaObjectData;
    type: AssetCategory | null;
    onClick: (asset: IotaObjectData) => void;
}

export function AssetTileLink({ asset, type, onClick }: AssetTileLinkProps): React.JSX.Element {
    function handleClick() {
        onClick(asset);
    }

    return (
        <>
            {type === AssetCategory.Visual ? (
                <VisualAssetTile asset={asset} icon={<VisibilityOff />} onClick={handleClick} />
            ) : (
                <NonVisualAssetCard asset={asset} />
            )}
        </>
    );
}
