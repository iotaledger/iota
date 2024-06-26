// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, Input } from '@/components';
import { Button } from '@/components/Buttons';
import { FlexDirection } from '@/lib/ui/enums';
import { useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema, ValidationError } from '@iota/core';

interface SendAssetPopupProps {
    asset: IotaObjectData;
    onClose?: () => void;
}

export default function SendAssetPopup({ asset, onClose }: SendAssetPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');
    const [errors, setErrors] = useState<string[]>([]);
    const activeAddress = useCurrentAccount()?.address;
    const schema = createNftSendValidationSchema(activeAddress || '', asset.objectId);

    async function handleAddressChange(address: string): Promise<void> {
        setRecipientAddress(address);

        try {
            await schema.validate({ to: address });
            setErrors([]);
        } catch (error) {
            if (error instanceof ValidationError) {
                setErrors(error.errors);
            }
        }
    }

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
                    onChange={(e) => handleAddressChange(e.target.value)}
                    label="Enter recipient address"
                    error={errors[0]}
                />
            </div>
            <Button onClick={handleSendAsset}>Send</Button>
            <Button onClick={onClose}>Cancel</Button>
        </div>
    );
}
