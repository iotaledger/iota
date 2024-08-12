// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress, isValidIotaAddress } from '@iota/iota-sdk/utils';

export function getOwnerDisplay(
    owner?: string,
    ownerType?: string,
    activeAddress?: string | null,
): {
    isOwner: boolean;
    ownerDisplay: string | null;
} {
    if (!owner)
        return {
            isOwner: false,
            ownerDisplay: '',
        };

    const isOwner = activeAddress === owner;

    let ownerDisplay = null;
    if (ownerType === 'Shared') {
        ownerDisplay = 'Shared';
    }

    if (isValidIotaAddress(owner)) {
        if (isOwner) {
            ownerDisplay = 'You';
        } else {
            ownerDisplay = formatAddress(owner);
        }
    }

    return {
        isOwner,
        ownerDisplay,
    };
}
