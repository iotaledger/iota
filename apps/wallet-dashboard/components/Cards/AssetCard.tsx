// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota.js/client';
import React from 'react';
import { Box, Button, ExternalImage, SendAssetPopup } from '@/components/index';
import { usePopups } from '@/hooks';

interface AssetCardProps {
    asset: IotaObjectData;
    showSendButton?: boolean;
}

function AssetCard({ asset, showSendButton }: AssetCardProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    function showSendAssetPopup(): void {
        openPopup(<SendAssetPopup asset={asset} onClose={closePopup} />);
    }
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
                    <p>Version: {asset.version}</p>
                </div>
            </div>
            {showSendButton && <Button onClick={showSendAssetPopup}>Send NFT</Button>}
        </Box>
    );
}

export default AssetCard;
