// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSuiClient } from '@mysten/dapp-kit';
import { CoinBalance } from '@mysten/sui.js/client';
import { MIST_PER_SUI } from '@mysten/sui.js/utils';
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

export function useBalance(coinType: string, address?: string | null) {
	const rpc = useSuiClient();
	const getBalanceQuery = useQuery<CoinBalance>({
		queryKey: ['get-balance', address, coinType],
		queryFn: async () => {
			return rpc.getBalance({
				owner: address!,
				coinType,
			});
		},
		enabled: !!address,
	});

	const calculateBalance = useMemo(() => {
		if (getBalanceQuery?.data?.totalBalance) {
			return Number(getBalanceQuery.data.totalBalance) / Number(MIST_PER_SUI);
		}
		return 0;
	}, [getBalanceQuery?.data?.totalBalance]);

	return {
		calculateBalance,
		getBalanceQuery,
	};
}
