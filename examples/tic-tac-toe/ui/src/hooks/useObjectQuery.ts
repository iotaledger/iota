// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext, useIotaClientQuery, UseIotaClientQueryOptions } from '@iota/dapp-kit';
import { GetObjectParams, IotaObjectResponse } from '@iota/iota-sdk/client';
import { useQueryClient, UseQueryResult } from '@tanstack/react-query';

export type UseObjectQueryOptions = UseIotaClientQueryOptions<'getObject', IotaObjectResponse>;
export type UseObjectQueryResponse = UseQueryResult<IotaObjectResponse, Error>;
export type InvalidateUseObjectQuery = () => void;

/**
 * Fetches an object, returning the response from RPC and a callback
 * to invalidate it.
 */
export function useObjectQuery(
	params: GetObjectParams,
	options?: UseObjectQueryOptions,
): [UseObjectQueryResponse, InvalidateUseObjectQuery] {
	const ctx = useIotaClientContext();
	const client = useQueryClient();
	const response = useIotaClientQuery('getObject', params, options);

	const invalidate = async () => {
		await client.invalidateQueries({
			queryKey: [ctx.network, 'getObject', params],
		});
	};

	return [response, invalidate];
}
