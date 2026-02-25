// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { useAccounts } from './useAccounts';

export interface AccountsInfo {
    isFirstAccount: boolean;
    totalAccounts: number;
}

/**
 * Hook that provides derived information about accounts.
 * Uses useAccounts under the hood and calculates additional metadata.
 */
export function useAccountsInfo(): AccountsInfo {
    const { data: accounts } = useAccounts();

    return useMemo(() => {
        const totalAccounts = accounts?.length ?? 0;
        return {
            isFirstAccount: totalAccounts === 0,
            totalAccounts,
        };
    }, [accounts]);
}
