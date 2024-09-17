// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaMoveObject, IotaObjectData, IotaObjectResponse } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

export const camelCase = (string: string) => string.replace(/(_\w)/g, (g) => g[1].toUpperCase());

export const parseObjectDataResponse = (response: IotaObjectResponse | undefined) =>
    ((response?.data as IotaObjectData)?.content as IotaMoveObject)?.fields as Record<string, any>;

export const parseRegistryResponse = (response: IotaObjectResponse | undefined): any => {
    const fields = parseObjectDataResponse(response)?.value?.fields || {};

    const object = Object.fromEntries(
        Object.entries({ ...fields }).map(([key, val]) => [camelCase(key), val]),
    );

    if (response?.data?.objectId) {
        object.id = response.data.objectId;
    }

    delete object.data;

    const data = (fields.data?.fields.contents || []).reduce(
        (acc: Record<string, any>, c: Record<string, any>) => {
            const key = c.fields.key;
            const value = c.fields.value;

            return {
                ...acc,
                [camelCase(key)]:
                    c.type.includes('Address') || key === 'addr'
                        ? normalizeIotaAddress(value)
                        : value,
            };
        },
        {},
    );

    return { ...object, ...data };
};
