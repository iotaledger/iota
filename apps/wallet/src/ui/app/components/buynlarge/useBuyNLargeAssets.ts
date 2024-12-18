// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';

import { useActiveAddress } from '../../hooks';
import { useConfig } from './useConfig';

export function useBuyNLargeAssets() {
	const config = useConfig();
	const address = useActiveAddress();
	const { data } = useIotaClientQuery(
		'getOwnedObjects',
		{
			owner: address ?? '',
			filter: { MatchAny: config.map(({ objectType }) => ({ StructType: objectType ?? '' })) },
			options: { showDisplay: true, showType: true },
		},
		{
			enabled: !!address && config.some(({ enabled }) => enabled),
		},
	);

	return config
		?.map((item) => {
			if (!item.enabled) return null;
			return {
				...item,
				asset: data?.data.find((x) => x.data?.type === item.objectType),
			};
		})
		.filter((x) => !!x);
}
