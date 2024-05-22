// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountSourceSerializedUI } from '_src/background/account-sources/AccountSource';
import { useQuery } from '@tanstack/react-query';

import { useBackgroundClient } from './useBackgroundClient';

export const accountSourcesQueryKey = ['background', 'client', 'account', 'sources'] as const;

export function useAccountSources() {
	const backgroundClient = useBackgroundClient();
	const res = useQuery({
		queryKey: accountSourcesQueryKey,
		queryFn: () => backgroundClient.getStoredEntities<AccountSourceSerializedUI>('accountSources'),
		gcTime: 30 * 1000,
		staleTime: 15 * 1000,
		refetchInterval: 30 * 1000,
		meta: { skipPersistedCache: true },
	});
	console.log('useAccountSources', res);
	return res;
}
