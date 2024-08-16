// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { useAccounts } from './useAccounts';
import type { BackgroundClient } from '_app/background-client';
import type { SerializedUIAccount } from '_src/background/accounts/Account';

export function useActiveAccount() {
    const { data: allAccounts } = useAccounts();
    return useMemo(() => {
        return getActiveAccount(allAccounts);
    }, [allAccounts]);
}

export async function getActiveAccountForce(backgroundClient: BackgroundClient) {
    const allAccounts = await backgroundClient.getStoredEntities<SerializedUIAccount>('accounts');

    return getActiveAccount(allAccounts);
}

function getActiveAccount(allAccounts: SerializedUIAccount[] | undefined) {
    if (!allAccounts) {
        return null;
    }
    const selected = allAccounts.find(({ selected }) => selected);
    if (selected) {
        return selected;
    }
    return allAccounts[0] || null;
}
