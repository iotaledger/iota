// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import { VisualAssetTile } from '../tiles';

interface AssetDetailsCardProps {
    asset: IotaObjectData;
}

export function VisualAssetDetailsCard({ asset }: AssetDetailsCardProps): React.JSX.Element {
    return (
        <div className="flex w-full gap-2">
            <VisualAssetTile asset={asset} />
            <div>
                <p>Digest: {asset.digest}</p>
                <p>Object ID: {asset.objectId}</p>
                {asset.type ? <p>Type: {asset.type}</p> : null}
                <p>Version: {asset.version}</p>
            </div>
        </div>
    );
}
