// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AddressInput } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAddress } from '_src/ui/app/hooks';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { useSigner } from '_src/ui/app/hooks/useSigner';
import { createNftSendValidationSchema, useGetKioskContents } from '@iota/core';
import { TransactionBlock } from '@iota/iota-sdk/transactions';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Field, Form, Formik } from 'formik';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';

import { useTransferKioskItem } from './useTransferKioskItem';
import { Button, ButtonHtmlType } from '@iota/apps-ui-kit';

interface TransferNFTFormProps {
    objectId: string;
    objectType?: string | null;
}

export function TransferNFTForm({ objectId, objectType }: TransferNFTFormProps) {
    const activeAddress = useActiveAddress();
    const validationSchema = createNftSendValidationSchema(activeAddress || '', objectId);
    const activeAccount = useActiveAccount();
    const signer = useSigner(activeAccount);
    const queryClient = useQueryClient();
    const navigate = useNavigate();
    const { data: kiosk } = useGetKioskContents(activeAddress);
    const transferKioskItem = useTransferKioskItem({ objectId, objectType });
    const isContainedInKiosk = kiosk?.list.some(
        (kioskItem) => kioskItem.data?.objectId === objectId,
    );

    const transferNFT = useMutation({
        mutationFn: async (to: string) => {
            if (!to || !signer) {
                throw new Error('Missing data');
            }

            if (isContainedInKiosk) {
                return transferKioskItem.mutateAsync({ to });
            }

            const tx = new TransactionBlock();
            tx.transferObjects([tx.object(objectId)], to);

            return signer.signAndExecuteTransactionBlock({
                transactionBlock: tx,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
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
            {({ isValid, touched, errors }) => (
                <Form autoComplete="off" className="h-full">
                    <div className="flex h-full flex-col justify-between">
                        <Field
                            component={AddressInput}
                            allowNegative={false}
                            name="to"
                            placeholder="Enter Address"
                        />

                        <Button
                            htmlType={ButtonHtmlType.Submit}
                            disabled={!isValid || !touched.to || !!errors.to}
                            text="Send"
                        />
                    </div>
                </Form>
            )}
        </Formik>
    );
}
