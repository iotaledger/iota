// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { normalizeStructTag } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

import { useAppsBackend } from '../../../../../core';

export function useBlockedObjectList() {
	const { request } = useAppsBackend();
	return useQuery({
		queryKey: ['apps-backend', 'guardian', 'object-list'],
		queryFn: () => request<{ blocklist: string[] }>('guardian/object-list'),
		select: (data) => data?.blocklist.map(normalizeStructTag) ?? [],
	});
}
