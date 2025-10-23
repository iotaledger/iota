// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { KioskClient, KioskItem, KioskOwnerCap } from '@iota/kiosk';
import { useQuery } from '@tanstack/react-query';

import { useKioskClient } from './useKioskClient';

export enum KioskTypes {
    IOTA = 'iota',
}

export type Kiosk = {
    items: KioskItem[];
    itemIds: string[];
    kioskId: string;
    type: KioskTypes;
    ownerCap?: KioskOwnerCap;
};

async function getIotaKioskContents(address: string, kioskClient: KioskClient) {
    const ownedKiosks = await kioskClient.getOwnedKiosks({ address });
    const contents = await Promise.all(
        ownedKiosks.kioskIds.map(async (id: string) => {
            const kiosk = await kioskClient.getKiosk({
                id,
                options: {
                    withObjects: true,
                    objectOptions: { showDisplay: true, showContent: true },
                },
            });
            return {
                itemIds: kiosk.itemIds,
                items: kiosk.items,
                kioskId: id,
                type: KioskTypes.IOTA,
                ownerCap: ownedKiosks.kioskOwnerCaps.find((k) => k.kioskId === id),
            };
        }),
    );
    return contents;
}

export function useGetKioskContents(address?: string | null) {
    const { network } = useIotaClientContext();
    const kioskClient = useKioskClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['get-kiosk-contents', address, network, kioskClient.network],
        queryFn: async () => {
            const iotaKiosks = await getIotaKioskContents(address!, kioskClient);
            return iotaKiosks;
        },
        select(data) {
            const kiosks = new Map<string, Kiosk>();
            const lookup = new Map<string, string>();

            data.forEach((kiosk) => {
                kiosks.set(kiosk.kioskId, kiosk);
                kiosk.itemIds.forEach((id) => {
                    lookup.set(id, kiosk.kioskId);
                });
            });

            return {
                kiosks,
                list: data.flatMap((kiosk) => kiosk.items),
                lookup,
            };
        },
        enabled: !!address,
    });
}
