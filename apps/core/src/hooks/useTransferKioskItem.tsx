// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import {
    useKioskClient,
    getKioskIdFromOwnerCap,
    KioskTypes,
    ORIGINBYTE_KIOSK_OWNER_TOKEN,
    useGetKioskContents,
    useGetObject,
    Feature,
    TransferAssetExecuteFn,
} from '../../';
import { useIotaClient } from '@iota/dapp-kit';
import { KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMutation } from '@tanstack/react-query';

export const ORIGINBYTE_PACKAGE_ID =
    '0x083b02db943238dcea0ff0938a54a17d7575f5b48034506446e501e963391480';

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
    const client = useIotaClient();
    const obPackageId = useFeatureValue(Feature.KioskOriginbytePackageId, ORIGINBYTE_PACKAGE_ID);
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

            if (kiosk.type === KioskTypes.ORIGINBYTE && objectData?.data?.data?.type) {
                const tx = new Transaction();
                const recipientKiosks = await client.getOwnedObjects({
                    owner: to,
                    options: { showContent: true },
                    filter: { StructType: ORIGINBYTE_KIOSK_OWNER_TOKEN },
                });
                const recipientKiosk = recipientKiosks.data[0];
                const recipientKioskId = recipientKiosk
                    ? getKioskIdFromOwnerCap(recipientKiosk)
                    : null;

                if (recipientKioskId) {
                    tx.moveCall({
                        target: `${obPackageId}::ob_kiosk::p2p_transfer`,
                        typeArguments: [objectType],
                        arguments: [
                            tx.object(kioskId),
                            tx.object(recipientKioskId),
                            tx.pure.id(objectId),
                        ],
                    });
                } else {
                    tx.moveCall({
                        target: `${obPackageId}::ob_kiosk::p2p_transfer_and_create_target_kiosk`,
                        typeArguments: [objectType],
                        arguments: [tx.object(kioskId), tx.pure.address(to), tx.pure.id(objectId)],
                    });
                }
                return executeFn({
                    transaction: tx,
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
