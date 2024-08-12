// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KioskListing, KioskOwnerCap } from '@iota/kiosk';
import { IotaObjectResponse } from '@iota/iota-sdk/client';
import { MICROS_PER_IOTA, normalizeIotaAddress } from '@iota/iota-sdk/utils';

// Parse the display of a list of objects into a simple {object_id: display} map
// to use throughout the app.
export const parseObjectDisplays = (
    data: IotaObjectResponse[],
): Record<string, Record<string, string> | undefined> => {
    return data.reduce<Record<string, Record<string, string> | undefined>>(
        (acc, item: IotaObjectResponse) => {
            const display = item.data?.display?.data;
            const id = item.data?.objectId;
            acc[id] = display || undefined;
            return acc;
        },
        {},
    );
};

export const processKioskListings = (data: KioskListing[]): Record<string, KioskListing> => {
    const results: Record<string, KioskListing> = {};

    data.filter((x) => !!x).map((x: KioskListing) => {
        results[x.objectId || ''] = x;
        return x;
    });
    return results;
};

export const microsToIota = (micros: bigint | string | undefined) => {
    if (!micros) return 0;
    return Number(micros || 0) / Number(MICROS_PER_IOTA);
};

export const formatIota = (amount: number) => {
    return new Intl.NumberFormat('en-US', {
        minimumFractionDigits: 2,
        maximumFractionDigits: 5,
    }).format(amount);
};

/**
 * Finds an active owner cap for a kioskId based on the
 * address owned kiosks.
 */
export const findActiveCap = (
    caps: KioskOwnerCap[] = [],
    kioskId: string,
): KioskOwnerCap | undefined => {
    return caps.find((x) => normalizeIotaAddress(x.kioskId) === normalizeIotaAddress(kioskId));
};
