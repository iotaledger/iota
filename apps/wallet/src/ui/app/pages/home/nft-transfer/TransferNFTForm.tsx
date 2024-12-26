// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useActiveAddress } from '_src/ui/app/hooks';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { useSigner } from '_src/ui/app/hooks/useSigner';
import { createNftSendValidationSchema, useGetKioskContents, AddressInput } from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Form, Formik } from 'formik';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';

import { useTransferKioskItem } from './useTransferKioskItem';
import { Button, ButtonHtmlType } from '@iota/apps-ui-kit';
import { Loader } from '@iota/ui-icons';
import {
    type IotaTransactionBlockResponse,
    type IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';

interface TransferNFTFormProps {
    objectId: string;
    objectType?: string | null;
}

export type ExecuteFn = (input: {
    transactionBlock: Uint8Array | Transaction;
    options?: IotaTransactionBlockResponseOptions;
}) => Promise<IotaTransactionBlockResponse>;

type ExecuteOnSuccessFn = (response: IotaTransactionBlockResponse) => void;

type ExecuteOnErrorFn = (error: Error) => void;

function useTransferAsset({
    objectId,
    objectType,
    activeAddress,
    executeFn,
    onSuccess,
    onError,
}: {
    objectId: string;
    objectType?: string | null;
    activeAddress?: string | null;
    executeFn?: ExecuteFn;
    onSuccess?: ExecuteOnSuccessFn;
    onError?: ExecuteOnErrorFn;
}) {
    const { data: kiosk } = useGetKioskContents(activeAddress);
    const transferKioskItem = useTransferKioskItem({
        objectId,
        objectType,
        executeFn,
        address: activeAddress,
    });
    const isContainedInKiosk = kiosk?.list.some(
        (kioskItem) => kioskItem.data?.objectId === objectId,
    );

    return useMutation({
        mutationFn: async (to: string) => {
            if (!to || !executeFn) {
                throw new Error('Missing data');
            }

            if (isContainedInKiosk) {
                return transferKioskItem.mutateAsync({ to });
            }

            const tx = new Transaction();
            tx.transferObjects([tx.object(objectId)], to);

            return executeFn({
                transactionBlock: tx,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
        onSuccess: onSuccess,
        onError: onError,
    });
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
        executeFn: signer?.signAndExecuteTransaction,
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
