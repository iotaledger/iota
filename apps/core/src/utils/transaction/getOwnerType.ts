// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectChange } from '@iota/iota-sdk/client';

export const getOwnerType = (change: IotaObjectChange) => {
    if (!('owner' in change)) return '';
    if (typeof change.owner === 'object') {
        if ('AddressOwner' in change.owner) return 'AddressOwner';
        if ('ObjectOwner' in change.owner) return 'ObjectOwner';
        if ('Shared' in change.owner) return 'Shared';
    }
    return change.owner;
};
