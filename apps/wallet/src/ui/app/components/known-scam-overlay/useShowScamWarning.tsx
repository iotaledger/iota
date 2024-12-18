// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { type Transaction } from '@iota/iota-sdk/transactions';
import { useEffect } from 'react';

import { useAppSelector } from '../../hooks';
import { API_ENV_TO_NETWORK, type RequestType } from './types';
import { useDappPreflight } from './useDappPreflight';

export function useShowScamWarning({
	url,
	requestType,
	transaction,
	requestId,
}: {
	url?: URL;
	requestType: RequestType;
	transaction?: Transaction;
	requestId: string;
}) {
	const { apiEnv } = useAppSelector((state) => state.app);
	const { data, isPending, isError } = useDappPreflight({
		requestType,
		origin: url?.origin,
		transaction,
		requestId,
		network: API_ENV_TO_NETWORK[apiEnv],
	});

	useEffect(() => {
		if (data?.block.enabled && url?.hostname) {
			ampli.interactedWithMaliciousDomain({ hostname: url.hostname });
		}
	}, [data, url]);

	return {
		data,
		isOpen: !!data?.block.enabled && !isError,
		isPending,
		isError,
	};
}
