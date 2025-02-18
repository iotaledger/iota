// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAccount, useSigner, useActiveAddress } from '_hooks';
import {
    createNftSendValidationSchema,
    AddressInput,
    useTransferAsset,
    type TransferAssetExecuteFn,
    useAssetGasBudgetEstimation,
    useFormatCoin,
    CoinFormat,
} from '@iota/core';
import { useQueryClient } from '@tanstack/react-query';
import { Form, Formik, useFormikContext } from 'formik';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import { Button, ButtonHtmlType, Divider, KeyValueInfo } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { type WalletSigner } from '_src/ui/app/walletSigner';

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
    const { values } = useFormikContext<{ to: string }>();
    const { data: gasBudgetEst } = useAssetGasBudgetEstimation({
        objectId,
        activeAddress,
        to: values?.to ?? '',
        objectType,
    });
    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: gasBudgetEst,
        format: CoinFormat.FULL,
    });
    return (
        <KeyValueInfo
            keyText={'Est. Gas Fees'}
            value={gasFormatted}
            supportingLabel={gasSymbol}
            fullwidth
        />
    );
}

export function TransferNFTForm({ objectId, objectType }: TransferNFTFormProps) {
    const activeAddress = useActiveAddress();
    const validationSchema = createNftSendValidationSchema(activeAddress || '', objectId);
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const queryClient = useQueryClient();
    const navigate = useNavigate();

    const transferNFT = useTransferAsset({
        activeAddress,
        objectId,
        objectType,
        executeFn: normalizeWalletSignAndExecute(signer),
        onSuccess: (response) => {
            queryClient.invalidateQueries({ queryKey: ['object', objectId] });
            queryClient.invalidateQueries({ queryKey: ['get-kiosk-contents'] });
            queryClient.invalidateQueries({ queryKey: ['get-owned-objects'] });

            ampli.sentCollectible({ objectId });

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

    return (
        <Formik
            initialValues={{
                to: '',
            }}
            validateOnChange
            validationSchema={validationSchema}
            onSubmit={({ to }) => transferNFT.mutateAsync(to)}
        >
            {({ isValid, dirty, isSubmitting }) => (
                <Form autoComplete="off" className="h-full">
                    <div className="flex h-full flex-col justify-between">
                        <div className="flex flex-col gap-y-sm">
                            <AddressInput name="to" placeholder="Enter Address" />
                            <Divider />
                            <GasBudgetComponent
                                objectId={objectId}
                                activeAddress={activeAddress}
                                objectType={objectType}
                            />
                        </div>

                        <Button
                            htmlType={ButtonHtmlType.Submit}
                            disabled={!(isValid && dirty) || isSubmitting}
                            text="Send"
                            icon={isSubmitting ? <Loader className="animate-spin" /> : undefined}
                            iconAfterText
                        />
                    </div>
                </Form>
            )}
        </Formik>
    );
}
