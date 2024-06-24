// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota.js/client';
import React from 'react';
import { Box } from '@/components/index';
import Image from 'next/image';

interface AssetCardProps {
    asset: IotaObjectData;
}

function AssetCard({ asset }: AssetCardProps): JSX.Element {
    const assetDisplayData = asset.display?.data;

    return (
        <Box>
            <div className="flex flex-col gap-2">
                {assetDisplayData && assetDisplayData.image && (
                    <Image
                        src={assetDisplayData.image}
                        alt={assetDisplayData.name}
                        width={80}
                        height={40}
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
