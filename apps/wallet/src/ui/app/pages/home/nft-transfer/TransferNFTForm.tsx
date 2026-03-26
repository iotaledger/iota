// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAccount, useSigner, useActiveAddress, useAppSelector } from '_hooks';
import {
    createNftSendValidationSchema,
    AddressInput,
    useTransferAsset,
    type TransferAssetExecuteFn,
    useAssetGasBudgetEstimation,
    useFormatCoin,
    toast,
    type SendNftFormValues,
    RECEIVING_ADDRESS_FIELD_IDS,
    useFeatureEnabledByNetwork,
    Feature,
} from '@iota/core';
import { CoinFormat } from '@iota/iota-sdk/utils';
import { useQueryClient } from '@tanstack/react-query';
import { Form, FormikProvider, useFormik, useFormikContext } from 'formik';
import { useNavigate } from 'react-router-dom';
import { Button, ButtonHtmlType, Divider, KeyValueInfo } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { type WalletSigner } from '_src/ui/app/walletSigner';
import { useMemo } from 'react';

interface TransferNFTFormProps {
    objectId: string;
    objectType?: string | null;
}

function normalizeWalletSignAndExecute(
    signer: WalletSigner | null,
): TransferAssetExecuteFn | undefined {
    if (!signer) return;

    const executeFn = signer.signAndExecuteTransaction.bind(signer);
    return ({ transaction, ...rest }) => executeFn({ transactionBlock: transaction, ...rest });
}

function GasBudgetComponent({
    objectId,
    activeAddress,
    objectType,
}: {
    objectId: string;
    activeAddress: string | null;
    objectType?: string | null;
}) {
    const { values, isValid } = useFormikContext<SendNftFormValues>();
    const recipientAddress = isValid ? values.resolvedAddress || values.to || '' : '';

    const { data: gasBudgetEst } = useAssetGasBudgetEstimation({
        objectId,
        activeAddress,
        to: recipientAddress,
        objectType,
    });
    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: gasBudgetEst,
        format: CoinFormat.Full,
    });
    return (
        <KeyValueInfo
            keyText={'Est. Gas Fees'}
            value={gasFormatted}
            supportingLabel={gasFormatted ? gasSymbol : undefined}
            fullwidth
        />
    );
}

export function TransferNFTForm({ objectId, objectType }: TransferNFTFormProps) {
    const activeAddress = useActiveAddress();
    const network = useAppSelector((state) => state.app.network);
    const isNameResolutionEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, network);

    const validationSchema = useMemo(
        () => createNftSendValidationSchema(activeAddress || '', objectId, isNameResolutionEnabled),
        [activeAddress, objectId, isNameResolutionEnabled],
    );
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const queryClient = useQueryClient();
    const navigate = useNavigate();

    const formik = useFormik<SendNftFormValues>({
        initialValues: {
            to: '',
            resolvedAddress: '',
        },
        validationSchema,
        onSubmit: handleSubmit,
        validateOnChange: false,
        validateOnBlur: false,
    });

    const transferNFT = useTransferAsset({
        activeAddress,
        objectId,
        objectType,
        executeFn: normalizeWalletSignAndExecute(signer),
        onSuccess: (response) => {
            queryClient.invalidateQueries({ queryKey: ['object', objectId] });
            queryClient.invalidateQueries({ queryKey: ['get-kiosk-contents'] });
            queryClient.invalidateQueries({ queryKey: ['get-owned-objects'] });

            ampli.sentCollectible({ collectibleType: objectType || undefined });

            return navigate(
                `/receipt?${new URLSearchParams({
                    txdigest: response.digest,
                    from: 'nfts',
                }).toString()}`,
            );
        },
        onError: (error) => {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
            );
        },
    });

    function handleSubmit(values: SendNftFormValues) {
        const recipient = values.resolvedAddress || values.to;
        transferNFT.mutate(recipient);
    }

    return (
        <FormikProvider value={formik}>
            <Form autoComplete="off" className="h-full">
                <div className="flex h-full flex-col justify-between">
                    <div className="flex flex-col gap-y-sm">
                        <AddressInput
                            {...RECEIVING_ADDRESS_FIELD_IDS}
                            placeholder="Enter Address"
                        />
                        <Divider />
                        <GasBudgetComponent
                            objectId={objectId}
                            activeAddress={activeAddress}
                            objectType={objectType}
                        />
                    </div>

                    <Button
                        htmlType={ButtonHtmlType.Submit}
                        disabled={!(formik.isValid && formik.dirty) || formik.isSubmitting}
                        text="Send"
                        icon={formik.isSubmitting ? <Loader className="animate-spin" /> : undefined}
                        iconAfterText
                    />
                </div>
            </Form>
        </FormikProvider>
    );
}
