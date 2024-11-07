// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AssetCategory } from '@/lib/enums';
import { VisibilityOff } from '@iota/ui-icons';
import { VisualAssetTile } from '.';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { NonVisualAssetCard } from './NonVisualAssetTile';

interface AssetTileProps {
    asset: IotaObjectData;
    type: AssetCategory;
    onClick?: () => void;
}

export function AssetTile({ asset, type, onClick }: AssetTileProps): React.JSX.Element {
    return type === AssetCategory.Visual ? (
        <VisualAssetTile asset={asset} icon={<VisibilityOff />} onClick={onClick} />
    ) : (
        <NonVisualAssetCard key={asset.digest} asset={asset} onClick={onClick} />
    );
}
