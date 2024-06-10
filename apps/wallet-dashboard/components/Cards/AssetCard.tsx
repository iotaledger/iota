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
    return (
        <Box>
            <div className="flex gap-2">
                {asset.display && asset.display.data && asset.display.data.image && (
                    <Image
                        src={asset.display.data.image}
                        alt={asset.display.data.name}
                        width={80}
                        height={40}
                    />
                )}
                <div>
                    <p>Digest: {asset.digest}</p>
                    <p>Object ID: {asset.objectId}</p>
                    <p>Version: {asset.version}</p>
                </div>
            </div>
        </Box>
    );
}

export default AssetCard;
