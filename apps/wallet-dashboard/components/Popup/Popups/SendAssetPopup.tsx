// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, Input } from '@/components';
import { Button } from '@/components/Buttons';
import { FlexDirection } from '@/lib/ui/enums';

interface SendAssetPopupProps {
    asset: IotaObjectData;
    onClose: () => void;
}

export default function SendAssetPopup({ asset, onClose }: SendAssetPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');

    function handleSendAsset(): void {
        console.log('Sending asset to: ', recipientAddress);
    }
    return (
        <div className="flex flex-col space-y-4">
            <AssetCard asset={asset} flexDirection={FlexDirection.Column} />
            <div className="flex flex-col space-y-2">
                <Input
                    type="text"
                    value={recipientAddress}
                    placeholder="Enter Address"
                    onChange={(e) => setRecipientAddress(e.target.value)}
                    label="Enter recipient address"
                />
            </div>
            <Button onClick={handleSendAsset}>Send</Button>
            <Button onClick={onClose}>Cancel</Button>
        </div>
    );
}
