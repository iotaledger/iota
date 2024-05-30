// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@mysten/dapp-kit';
import { IotaObjectDataOptions, IotaObjectResponse } from '@mysten/iota.js/client';
import { useQuery, UseQueryOptions } from '@tanstack/react-query';

import { chunkArray } from '../utils/chunkArray';

export function useMultiGetObjects(
	ids: string[],
	options: IotaObjectDataOptions,
	queryOptions?: Omit<UseQueryOptions<IotaObjectResponse[]>, 'queryKey' | 'queryFn'>,
) {
	const client = useIotaClient();
	return useQuery({
		...queryOptions,
		queryKey: ['multiGetObjects', ids],
		queryFn: async () => {
			const responses = await Promise.all(
				chunkArray(ids, 50).map((chunk) =>
					client.multiGetObjects({
						ids: chunk,
						options,
					}),
				),
			);
			return responses.flat();
		},
		enabled: !!ids?.length,
	});
}
