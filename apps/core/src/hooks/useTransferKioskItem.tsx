// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useKioskClient,
    KioskTypes,
    useGetKioskContents,
    useGetObject,
    TransferAssetExecuteFn,
} from '../../';
import { KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMutation } from '@tanstack/react-query';

export function useTransferKioskItem({
    objectId,
    objectType,
    executeFn,
    address,
}: {
    objectId: string;
    objectType?: string | null;
    executeFn?: TransferAssetExecuteFn;
    address?: string | null;
}) {
    const { data: kioskData } = useGetKioskContents(address); // show personal kiosks too
    const objectData = useGetObject(objectId);
    const kioskClient = useKioskClient();

    return useMutation({
        mutationFn: async ({ to }: { to: string }) => {
            if (!to || !executeFn || !objectType) {
                throw new Error('Missing data');
            }

            const kioskId = kioskData?.lookup.get(objectId);
            const kiosk = kioskData?.kiosks.get(kioskId!);

            if (!kioskId || !kiosk) {
                throw new Error('Failed to find object in a kiosk');
            }

            if (kiosk.type === KioskTypes.IOTA && objectData?.data?.data?.type && kiosk?.ownerCap) {
                const txb = new Transaction();

                new KioskTransaction({ transaction: txb, kioskClient, cap: kiosk.ownerCap })
                    .transfer({
                        itemType: objectData.data.data.type as string,
                        itemId: objectId,
                        address: to,
                    })
                    .finalize();

                return executeFn({
                    transaction: txb,
                    options: {
                        showInput: true,
                        showEffects: true,
                        showEvents: true,
                    },
                });
            }

            throw new Error('Failed to transfer object');
        },
    });
}
