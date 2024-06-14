// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { ExternalImage } from '@/components';
import { Button } from '@/components/Buttons';

interface SendAssetPopupProps {
    asset: IotaObjectData;
    onClose?: () => void;
}

export default function SendAssetPopup({ asset, onClose }: SendAssetPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');

    function handleSendAsset(): void {
        console.log('Sending asset to: ', recipientAddress);
    }
    return (
        <div className="flex flex-col space-y-4">
            <div className="flex min-w-[40vw] flex-col items-center space-y-4">
                {asset.display && asset.display.data && asset.display.data.image_url && (
                    <ExternalImage
                        src={asset.display.data.image_url}
                        alt={asset.display.data.name}
                        width={200}
                        height={200}
                        className="object-cover"
                    />
                )}
                <div className="flex w-full flex-col space-y-1">
                    <h4 className="text-center font-semibold">Object Details</h4>
                    <div className="flex w-full flex-col items-start">
                        <div>
                            <span className="font-semibold">Digest:</span> {asset.digest}
                        </div>
                        <div>
                            <span className="font-semibold">Object ID:</span> {asset.objectId}
                        </div>
                        <div>
                            <span className="font-semibold">Version:</span> {asset.version}
                        </div>
                    </div>
                </div>
            </div>
            <div className="flex flex-col space-y-2">
                <label htmlFor="recipientAddress">Enter recipient address</label>
                <input
                    className="w-full rounded-md border border-gray-300 p-2"
                    type="text"
                    value={recipientAddress}
                    name="recipientAddress"
                    id="recipientAddress"
                    placeholder="0x..."
                    autoComplete="off"
                    onChange={(e) => setRecipientAddress(e.target.value)}
                />
            </div>
            <Button onClick={handleSendAsset}>Send</Button>
            <Button onClick={onClose}>Cancel</Button>
        </div>
    );
}
