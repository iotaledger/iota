// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { type TransactionStatus } from '_src/shared/qredo-api';
import { useQuery } from '@tanstack/react-query';

import { useActiveAddress } from './useActiveAddress';
import useAppSelector from './useAppSelector';
import { useQredoAPI } from './useQredoAPI';

export function useGetQredoTransactions({
	qredoID,
	filterStatus,
	forceDisabled,
}: {
	qredoID?: string;
	filterStatus?: TransactionStatus[];
	forceDisabled?: boolean;
}) {
	const [qredoAPI] = useQredoAPI(qredoID);
	const network = useAppSelector(({ app: { network } }) => network);
	const activeAddress = useActiveAddress();
	return useQuery({
		queryKey: [
			'get',
			'qredo',
			'transacions',
			qredoAPI,
			qredoID,
			network,
			activeAddress,
			filterStatus,
		],
		queryFn: () =>
			qredoAPI!.getTransactions({
				network,
				address: activeAddress!,
			}),
		select: ({ list }) =>
			list.filter(({ status }) => !filterStatus?.length || filterStatus.includes(status)),
		enabled: !!(qredoAPI && qredoID && network && activeAddress && !forceDisabled),
		staleTime: 5000,
		refetchInterval: 5000,
	});
}
