// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota-sdk/client';
import React from 'react';
import { Box, ExternalImage } from '@/components/index';
import { useGetNFTMeta } from '@iota/core';
import { FlexDirection } from '@/lib/ui/enums';

interface AssetCardProps {
    asset: IotaObjectData;
    flexDirection?: FlexDirection;
}

function AssetCard({ asset, flexDirection }: AssetCardProps): React.JSX.Element {
    const { data: nftMeta } = useGetNFTMeta(asset.objectId);
    return (
        <Box>
            <div className={`flex ${flexDirection} w-full gap-2`}>
                {asset.display && nftMeta && nftMeta.imageUrl && (
                    <ExternalImage
                        src={nftMeta.imageUrl}
                        alt={nftMeta.name ?? asset.display.data?.name}
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
