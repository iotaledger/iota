// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useAppsBackend } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { type Transaction } from '@iota/iota-sdk/transactions';
import { toBase64 } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

import {
	RequestType,
	type DappPreflightRequest,
	type DappPreflightResponse,
	type Network,
} from './types';

export function useDappPreflight({
	requestType,
	origin,
	transaction,
	requestId,
	network,
}: {
	requestType: RequestType;
	origin?: string;
	transaction?: Transaction;
	requestId: string;
	network: Network;
}) {
	const { request } = useAppsBackend();
	const client = useIotaClient();

	return useQuery({
		// eslint-disable-next-line @tanstack/query/exhaustive-deps
		queryKey: ['dapp-preflight', { requestId, requestType, origin }],
		queryFn: async () => {
			if (!origin) {
				throw new Error('No origin provided');
			}

			const body: DappPreflightRequest = {
				network,
				requestType,
				origin,
			};

			if (requestType === RequestType.SIGN_TRANSACTION && transaction) {
				const transactionBytes = await transaction.build({ client });
				body.transactionBytes = toBase64(transactionBytes);
			}

			return request<DappPreflightResponse>(
				'v1/dapp-preflight',
				{},
				{
					method: 'POST',
					body: JSON.stringify(body),
					headers: { 'Content-Type': 'application/json' },
				},
			);
		},
		enabled: !!origin,
		staleTime: 5 * 60 * 1000,
	});
}
