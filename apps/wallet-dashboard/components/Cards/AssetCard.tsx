// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota.js/client';
import React from 'react';
import { Box, ExternalImage } from '@/components/index';

interface AssetCardProps {
    asset: IotaObjectData;
}

function AssetCard({ asset }: AssetCardProps): React.JSX.Element {
    return (
        <Box>
            <div className="flex gap-2">
                {asset.display && asset.display.data && asset.display.data.image_url && (
                    <ExternalImage
                        src={asset.display.data.image_url}
                        alt={asset.display.data.name}
                        width={80}
                        height={80}
                        className="object-cover"
                    />
                )}
                <div>
                    <p>Digest: {asset.digest}</p>
                    <p>Object ID: {asset.objectId}</p>
                    {asset.type ? <p>Type: {asset.type}</p> : null}
                    <p>Version: {asset.version}</p>
                </div>
            </div>
        </Box>
    );
}

export default AssetCard;
