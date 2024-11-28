// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema } from '@iota/core';
import { DetailsView, SendView } from './views';
import { AssetsDialogView } from './interfaces';
import { useCreateSendAssetTransaction, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import { useRouter } from 'next/navigation';

interface AssetsDialogProps {
    onClose: () => void;
    view: AssetsDialogView;
    setView: (view: AssetsDialogView) => void;
}

export interface FormValues {
    to: string;
}

const INITIAL_VALUES: FormValues = {
    to: '',
};

export function AssetsDialog({ onClose, setView, view }: AssetsDialogProps): JSX.Element {
    const router = useRouter();
    const account = useCurrentAccount();
    const { addNotification } = useNotifications();

    const isOpen = !!view.asset?.objectId;
    const activeAddress = account?.address ?? '';
    const objectId = view.asset?.objectId ?? '';

    const validationSchema = createNftSendValidationSchema(activeAddress, objectId);

    function onDetailsSend() {
        if (view.type === 'details') {
            setView({
                ...view,
                type: 'send',
            });
        }
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

    function onSendBack() {
        if (view.type === 'send') {
            setView({
                ...view,
                type: 'details',
            });
        }
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => onClose()}>
            <FormikProvider value={formik}>
                <>
                    {view.type === 'details' ? (
                        <DetailsView asset={view.asset} onClose={onClose} onSend={onDetailsSend} />
                    ) : undefined}
                    {view.type === 'send' ? (
                        <SendView asset={view.asset} onClose={onClose} onBack={onSendBack} />
                    ) : undefined}
                </>
            </FormikProvider>
        </Dialog>
    );
}
