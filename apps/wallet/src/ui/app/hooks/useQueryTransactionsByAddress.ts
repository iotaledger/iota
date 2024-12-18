// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FEATURES } from '_src/shared/experimentation/features';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export function useQueryTransactionsByAddress(address: string | null) {
	const rpc = useIotaClient();
	const refetchInterval = useFeatureValue(FEATURES.WALLET_ACTIVITY_REFETCH_INTERVAL, 20_000);

	return useQuery({
		queryKey: ['transactions-by-address', address],
		queryFn: async () => {
			// combine from and to transactions
			const [txnIds, fromTxnIds] = await Promise.all([
				rpc.queryTransactionBlocks({
					filter: {
						ToAddress: address!,
					},
					options: {
						showInput: true,
						showEffects: true,
						showEvents: true,
					},
				}),
				rpc.queryTransactionBlocks({
					filter: {
						FromAddress: address!,
					},
					options: {
						showInput: true,
						showEffects: true,
						showEvents: true,
					},
				}),
			]);

			const inserted = new Map();
			const uniqueList: IotaTransactionBlockResponse[] = [];

			[...txnIds.data, ...fromTxnIds.data]
				.sort((a, b) => Number(b.timestampMs ?? 0) - Number(a.timestampMs ?? 0))
				.forEach((txb) => {
					if (inserted.get(txb.digest)) return;
					uniqueList.push(txb);
					inserted.set(txb.digest, true);
				});

			return uniqueList;
		},
		enabled: !!address,
		staleTime: 10 * 1000,
		refetchInterval,
	});
}
