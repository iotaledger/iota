// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useCallback, useState } from 'react';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetCard, Input } from '@/components';
import { Button } from '@/components/Buttons';
import { FlexDirection } from '@/lib/ui/enums';
import { useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema, ValidationError } from '@iota/core';
import { useRouter } from 'next/navigation';
import { useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { useCreateSendAssetTransaction } from '@/hooks';

interface SendAssetPopupProps {
    asset: IotaObjectData;
    onClose: () => void;
}

export default function SendAssetPopup({ asset, onClose }: SendAssetPopupProps): JSX.Element {
    const [recipientAddress, setRecipientAddress] = useState<string>('');
    const [errors, setErrors] = useState<string[]>([]);
    const activeAddress = useCurrentAccount()?.address;
    const router = useRouter();
    const { addNotification } = useNotifications();
    const { mutation: sendAsset } = useCreateSendAssetTransaction(
        asset.objectId,
        onSendAssetSuccess,
        onSendAssetError,
    );

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

    function onSendAssetSuccess() {
        addNotification('Transfer transaction successful', NotificationType.Success);
        onClose?.();
        router.push('/dashboard/assets/visual-assets');
    }

    function onSendAssetError() {
        addNotification('Transfer transaction failed', NotificationType.Error);
        onClose?.();
    }

    const handleSendAsset = useCallback(async () => {
        try {
            await sendAsset.mutateAsync(recipientAddress);
        } catch (error) {
            addNotification('Transfer transaction failed', NotificationType.Error);
        }
    }, [recipientAddress, sendAsset, addNotification]);

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
