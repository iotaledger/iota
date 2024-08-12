// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { DEFAULT_SORT_ORDER, groupByType } from '_app/helpers/accounts';
import { useAccounts } from './useAccounts';

export function useAccountGroups() {
    const { data: accounts } = useAccounts();

    const sortedAndGroupedAccounts = useMemo(() => {
        return groupByType(accounts ?? []);
    }, [accounts]);

    const list = () => {
        return DEFAULT_SORT_ORDER.flatMap((type) => {
            const group = sortedAndGroupedAccounts[type];
            return Object.values(group).flat();
        });
    };

    return { ...sortedAndGroupedAccounts, list };
}
