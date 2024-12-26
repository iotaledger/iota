// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useGetKioskContents, TransferAssetExecuteFn } from '../';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMutation } from '@tanstack/react-query';
import { useTransferKioskItem } from './useTransferKioskItem';

export function useTransferAsset<T extends IotaTransactionBlockResponse>({
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
    executeFn?: TransferAssetExecuteFn<T>;
    onSuccess?: (response: IotaTransactionBlockResponse, variables: string) => void;
    onError?: (error: Error) => void;
}) {
    const { data: kiosk } = useGetKioskContents(activeAddress);
    const transferKioskItem = useTransferKioskItem<T>({
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
                transaction: tx,
                options: {
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
        onSuccess,
        onError,
    });
}
