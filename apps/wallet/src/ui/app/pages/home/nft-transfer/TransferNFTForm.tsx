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
} from '@iota/core';
import { useQueryClient } from '@tanstack/react-query';
import { Form, Formik } from 'formik';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import { Button, ButtonHtmlType } from '@iota/apps-ui-kit';
import { Loader } from '@iota/ui-icons';
import { type WalletSigner } from '_src/ui/app/walletSigner';

interface TransferNFTFormProps {
    objectId: string;
    objectType?: string | null;
}

function normalizeWalletSignAndExecute(
    signer: WalletSigner | null,
): TransferAssetExecuteFn | undefined {
    if (!signer || !signer) return;

    const executeFn = signer.signAndExecuteTransaction.bind(signer);
    return ({ transaction, ...rest }) => executeFn({ transactionBlock: transaction, ...rest });
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
                        <AddressInput name="to" placeholder="Enter Address" />

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
