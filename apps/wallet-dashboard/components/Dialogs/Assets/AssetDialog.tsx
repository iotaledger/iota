// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useIotaClient, useCurrentAccount } from '@iota/dapp-kit';
import { createNftSendValidationSchema } from '@iota/core';
import { DetailsView, SendView } from './views';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './constants';
import { useCreateSendAssetTransaction } from '@/hooks';
import { TransactionDetailsView } from '../SendToken';
import { DialogLayout } from '../layout';
import toast from 'react-hot-toast';

interface AssetsDialogProps {
    onClose: () => void;
    asset: IotaObjectData;
    refetchAssets: () => void;
}

interface FormValues {
    to: string;
}

const INITIAL_VALUES: FormValues = {
    to: '',
};

export function AssetDialog({ onClose, asset, refetchAssets }: AssetsDialogProps): JSX.Element {
    const [view, setView] = useState<AssetsDialogView>(AssetsDialogView.Details);
    const account = useCurrentAccount();
    const [digest, setDigest] = useState<string>('');
    const activeAddress = account?.address ?? '';
    const objectId = asset?.objectId ?? '';
    const iotaClient = useIotaClient();
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
            const executed = await sendAsset.mutateAsync(values.to);

            const tx = await iotaClient.waitForTransaction({
                digest: executed.digest,
            });

            setDigest(tx.digest);
            refetchAssets();
            toast.success('Transfer transaction successful');
            setView(AssetsDialogView.TransactionDetails);
        } catch {
            toast.error('Transfer transaction failed');
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
            <DialogLayout>
                <>
                    {view === AssetsDialogView.Details && (
                        <DetailsView asset={asset} onClose={onOpenChange} onSend={onDetailsSend} />
                    )}
                    {view === AssetsDialogView.Send && (
                        <FormikProvider value={formik}>
                            <SendView
                                asset={asset}
                                onClose={onOpenChange}
                                onBack={onSendViewBack}
                            />
                        </FormikProvider>
                    )}

                    {view === AssetsDialogView.TransactionDetails && !!digest ? (
                        <TransactionDetailsView digest={digest} onClose={onOpenChange} />
                    ) : null}
                </>
            </DialogLayout>
        </Dialog>
    );
}
