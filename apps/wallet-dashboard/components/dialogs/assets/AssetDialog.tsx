// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState } from 'react';
import { Dialog } from '@iota/apps-ui-kit';
import { FormikProvider, useFormik } from 'formik';
import { useIotaClient, useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import {
    createNftSendValidationSchema,
    useTransferAsset,
    isKioskOwnerToken,
    useKioskClient,
    useNftDetails,
    toast,
    SendNftFormValues,
    useFeatureEnabledByNetwork,
    Feature,
} from '@iota/core';
import { DetailsView, SendView, KioskDetailsView } from './views';
import { getNetwork, IotaObjectData, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { AssetsDialogView } from './constants';
import { TransactionDetailsView } from '../send-token';
import { DialogLayout } from '../layout';
import { ampli } from '@/lib/utils/analytics';
import { shouldResolveInputAsName } from '@iota/core/utils/validation/names';
import { useNetwork } from '@iota/core/src/hooks/useNetwork';

interface AssetsDialogProps {
    onClose: () => void;
    asset: IotaObjectData;
    refetchAssets: () => void;
}

const INITIAL_VALUES: SendNftFormValues = {
    to: '',
    resolvedAddress: '',
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
    const [chosenKioskAsset, setChosenKioskAsset] = useState<IotaObjectData | null>(null);
    const [digest, setDigest] = useState<string>('');

    const activeAsset = chosenKioskAsset || asset;
    const objectId = chosenKioskAsset ? chosenKioskAsset.objectId : asset ? asset.objectId : '';

    const networkId = useNetwork();
    const network = getNetwork(networkId).id;
    const isNameResolutionEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    const validationSchema = useMemo(
        () => createNftSendValidationSchema(activeAddress, objectId, isNameResolutionEnabled),
        [activeAddress, objectId, isNameResolutionEnabled],
    );
    const { objectData } = useNftDetails(objectId, activeAddress);

    const { mutateAsync: sendAsset } = useTransferAsset({
        objectId,
        objectType: objectData?.type,
        activeAddress: activeAddress,
        executeFn: signAndExecuteTransaction,
    });

    const formik = useFormik<SendNftFormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema,
        onSubmit: onSubmit,
        validateOnChange: false,
        validateOnBlur: false,
    });

    async function onSubmit(values: SendNftFormValues) {
        try {
            const isNameInput = shouldResolveInputAsName(values.to);
            const executed = await sendAsset(
                isNameInput ? (values.resolvedAddress ?? '') : values.to,
            );

            const tx = await iotaClient.waitForTransaction({
                digest: executed.digest,
            });

            setDigest(tx.digest);
            refetchAssets();
            toast.success('Transfer transaction successful');
            setView(AssetsDialogView.TransactionDetails);
            ampli.sentCollectible({
                collectibleType: objectData?.type || undefined,
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
        setChosenKioskAsset(null);
        onClose();
    }

    function onKioskItemClick(item: IotaObjectData) {
        setChosenKioskAsset(item);
        setView(AssetsDialogView.Details);
    }

    function onBack() {
        if (!chosenKioskAsset) {
            onClose();
        }
        setChosenKioskAsset(null);
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
                                objectType={objectData?.type ?? ''}
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
