// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { useQuery } from '@tanstack/react-query';

import { ACCOUNTS_QUERY_KEY } from '../helpers/queryClientKeys';
import { useBackgroundClient } from './useBackgroundClient';

export function useAccounts() {
    const backgroundClient = useBackgroundClient();
    return useQuery({
        queryKey: ACCOUNTS_QUERY_KEY,
        queryFn: () => backgroundClient.getStoredEntities<SerializedUIAccount>('accounts'),
        gcTime: 30 * 1000,
        staleTime: 15 * 1000,
        refetchInterval: 30 * 1000,
        meta: { skipPersistedCache: true },
    });
}
