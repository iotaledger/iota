// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountType } from '_src/background/accounts/account';
import { useMemo } from 'react';

import { useAccounts } from './useAccounts';

export function useCountAccountsByType() {
    const { data: accounts, isPending } = useAccounts();
    const countPerType = useMemo(
        () =>
            accounts?.reduce<Partial<Record<AccountType, { total: number }>>>((acc, anAccount) => {
                acc[anAccount.type] = acc[anAccount.type] || { total: 0 };
                acc[anAccount.type]!.total++;
                return acc;
            }, {}) || {},
        [accounts],
    );
    return { data: countPerType, isPending };
}
