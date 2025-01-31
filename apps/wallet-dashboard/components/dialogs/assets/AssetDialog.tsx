// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useIotaClient, useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import {
    createNftSendValidationSchema,
    useTransferAsset,
    isKioskOwnerToken,
    useKioskClient,
    useNftDetails,
} from '@iota/core';
import { DetailsView, SendView, KioskDetailsView } from './views';
import { IotaObjectData, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './constants';
import { TransactionDetailsView } from '../send-token';
import { DialogLayout } from '../layout';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';

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
    const kioskClient = useKioskClient();
    const account = useCurrentAccount();
    const iotaClient = useIotaClient();
    const { mutateAsync: signAndExecuteTransaction } =
        useSignAndExecuteTransaction<IotaTransactionBlockResponse>();

    const isTokenOwnedByKiosk = isKioskOwnerToken(kioskClient.network, asset);
    const activeAddress = account?.address ?? '';

    const initView = isTokenOwnedByKiosk ? AssetsDialogView.KioskDetails : AssetsDialogView.Details;

    const [view, setView] = useState<AssetsDialogView>(initView);
    const [chosenKioskAsset, setChoosenKioskAsset] = useState<IotaObjectData | null>(null);
    const [digest, setDigest] = useState<string>('');

    const activeAsset = chosenKioskAsset || asset;
    const objectId = chosenKioskAsset ? chosenKioskAsset.objectId : asset ? asset.objectId : '';
    const validationSchema = createNftSendValidationSchema(activeAddress, objectId);
    const { objectData } = useNftDetails(objectId, activeAddress);

    const { mutateAsync: sendAsset } = useTransferAsset({
        objectId,
        objectType: objectData?.type,
        activeAddress: activeAddress,
        executeFn: signAndExecuteTransaction,
    });

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: onSubmit,
        validateOnChange: true,
    });

    async function onSubmit(values: FormValues) {
        try {
            const executed = await sendAsset(values.to);

            const tx = await iotaClient.waitForTransaction({
                digest: executed.digest,
            });

            setDigest(tx.digest);
            refetchAssets();
            toast.success('Transfer transaction successful');
            setView(AssetsDialogView.TransactionDetails);
            ampli.sentCollectible({
                objectId,
            });
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
        setChoosenKioskAsset(null);
        onClose();
    }

    function onKioskItemClick(item: IotaObjectData) {
        setChoosenKioskAsset(item);
        setView(AssetsDialogView.Details);
    }

    function onBack() {
        if (!chosenKioskAsset) {
            return;
        }
        setChoosenKioskAsset(null);
        setView(AssetsDialogView.KioskDetails);
    }

    return (
        <Dialog open onOpenChange={onOpenChange}>
            <DialogLayout>
                <>
                    {view === AssetsDialogView.KioskDetails && (
                        <KioskDetailsView
                            asset={activeAsset}
                            onClose={onOpenChange}
                            onItemClick={onKioskItemClick}
                        />
                    )}
                    {view === AssetsDialogView.Details && (
                        <DetailsView
                            asset={activeAsset}
                            onClose={onOpenChange}
                            onSend={onDetailsSend}
                            onBack={onBack}
                        />
                    )}
                    {view === AssetsDialogView.Send && (
                        <FormikProvider value={formik}>
                            <SendView
                                objectId={objectId}
                                senderAddress={activeAddress}
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
