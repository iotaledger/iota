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

export function AssetDialog({ onClose: onCloseCb, asset }: AssetsDialogProps): JSX.Element {
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
            setView(AssetsDialogView.Details);
            onCloseCb();
            addNotification('Transfer transaction successful', NotificationType.Success);
        } catch (error) {
            addNotification('Transfer transaction failed', NotificationType.Error);
        }
    }

    function onDetailsSend() {
        setView(AssetsDialogView.Send);
    }

    function onSendViewBack() {
        setView(AssetsDialogView.Details);
    }
    function onClose() {
        setView(AssetsDialogView.Details);
        onCloseCb();
    }
    return (
        <Dialog open onOpenChange={onClose}>
            <FormikProvider value={formik}>
                <>
                    {view === AssetsDialogView.Details && (
                        <DetailsView asset={asset} onClose={onClose} onSend={onDetailsSend} />
                    )}
                    {view === AssetsDialogView.Send && (
                        <SendView asset={asset} onClose={onClose} onBack={onSendViewBack} />
                    )}
                </>
            </FormikProvider>
        </Dialog>
    );
}
