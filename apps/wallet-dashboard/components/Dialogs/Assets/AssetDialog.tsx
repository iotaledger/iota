// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema } from '@iota/core';
import { DetailsView, SendView } from './views';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './constants';
import { useCreateSendAssetTransaction, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';

interface AssetsDialogProps {
    onClose: () => void;
    asset: IotaObjectData;
}

interface FormValues {
    to: string;
}

const INITIAL_VALUES: FormValues = {
    to: '',
};

export function AssetDialog({ onClose, asset }: AssetsDialogProps): JSX.Element {
    const [view, setView] = useState<AssetsDialogView>(AssetsDialogView.Details);
    const account = useCurrentAccount();
    const activeAddress = account?.address ?? '';
    const objectId = asset?.objectId ?? '';
    const { addNotification } = useNotifications();
    const validationSchema = createNftSendValidationSchema(activeAddress, objectId);

    const { mutation: sendAsset } = useCreateSendAssetTransaction(objectId);

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: onSubmit,
        validateOnChange: true,
    });

    async function onSubmit(values: FormValues) {
        try {
            await sendAsset.mutateAsync(values.to);
            addNotification('Transfer transaction successful', NotificationType.Success);
            onClose();
            setView(AssetsDialogView.Details);
        } catch {
            addNotification('Transfer transaction failed', NotificationType.Error);
        }
    }

    function onDetailsSend() {
        setView(AssetsDialogView.Send);
    }

    function onSendViewBack() {
        setView(AssetsDialogView.Details);
    }
    function onOpenChange() {
        setView(AssetsDialogView.Details);
        onClose();
    }
    return (
        <Dialog open onOpenChange={onOpenChange}>
            <FormikProvider value={formik}>
                <>
                    {view === AssetsDialogView.Details && (
                        <DetailsView asset={asset} onClose={onOpenChange} onSend={onDetailsSend} />
                    )}
                    {view === AssetsDialogView.Send && (
                        <SendView asset={asset} onClose={onOpenChange} onBack={onSendViewBack} />
                    )}
                </>
            </FormikProvider>
        </Dialog>
    );
}
