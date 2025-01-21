// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { isKioskOwnerToken, useKioskClient, KioskTile, AssetCategory } from '@iota/core';
import { VisibilityOff } from '@iota/apps-ui-icons';
import { VisualAssetTile } from '.';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { NonVisualAssetCard } from './NonVisualAssetTile';
import { useCurrentAccount } from '@iota/dapp-kit';

interface AssetTileLinkProps {
    asset: IotaObjectData;
    type: AssetCategory | null;
    onClick: (asset: IotaObjectData) => void;
}

export function AssetTileLink({ asset, type, onClick }: AssetTileLinkProps): React.JSX.Element {
    const account = useCurrentAccount();
    const kioskClient = useKioskClient();
    const isTokenOwnedByKiosk = isKioskOwnerToken(kioskClient.network, asset);
    function handleClick() {
        onClick(asset);
    }

    return (
        <>
            {type === AssetCategory.Visual && isTokenOwnedByKiosk ? (
                <KioskTile object={asset} address={account?.address} onClick={handleClick} />
            ) : type === AssetCategory.Visual ? (
                <VisualAssetTile asset={asset} icon={<VisibilityOff />} onClick={handleClick} />
            ) : (
                <NonVisualAssetCard asset={asset} />
            )}
        </>
    );
}
