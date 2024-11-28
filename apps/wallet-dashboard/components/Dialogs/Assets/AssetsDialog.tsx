// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema } from '@iota/core';
import { DetailsView, SendView } from './views';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './constants';
import { useCreateSendAssetTransaction, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { useRouter } from 'next/navigation';

interface AssetsDialogProps {
    isOpen: boolean;
    onClose: () => void;
    asset: IotaObjectData | null;
    view: AssetsDialogView;
    setView: (view: AssetsDialogView | undefined) => void;
}

export interface FormValues {
    to: string;
}

const INITIAL_VALUES: FormValues = {
    to: '',
};

export function AssetsDialog({
    isOpen,
    onClose,
    asset,
    setView,
    view,
}: AssetsDialogProps): JSX.Element {
    const account = useCurrentAccount();
    const activeAddress = account?.address ?? '';
    const objectId = asset?.objectId ?? '';
    const router = useRouter();
    const { addNotification } = useNotifications();
    const validationSchema = createNftSendValidationSchema(activeAddress, objectId);

    function onDetailsSend() {
        setView(AssetsDialogView.Send);
    }

    const { mutation: sendAsset } = useCreateSendAssetTransaction(
        objectId,
        onSendAssetSuccess,
        onSendAssetError,
    );

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: onSubmit,
        validateOnChange: true,
    });

    function onSendAssetSuccess() {
        addNotification('Transfer transaction successful', NotificationType.Success);
        router.push(ASSETS_ROUTE.path + '/assets');
    }

    function onSendAssetError() {
        addNotification('Transfer transaction failed', NotificationType.Error);
    }

    async function onSubmit(values: FormValues) {
        try {
            await sendAsset.mutateAsync(values.to);
        } catch (error) {
            addNotification('Transfer transaction failed', NotificationType.Error);
        }
    }

    function onSendClose() {
        setView(undefined);
    }

    function onSendBack() {
        setView(AssetsDialogView.Details);
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => onClose()}>
            <FormikProvider value={formik}>
                <>
                    {view === AssetsDialogView.Details && asset && (
                        <DetailsView asset={asset} onClose={onClose} onSend={onDetailsSend} />
                    )}
                    {view === AssetsDialogView.Send && asset && (
                        <SendView asset={asset} onClose={onSendClose} onBack={onSendBack} />
                    )}
                </>
            </FormikProvider>
        </Dialog>
    );
}
