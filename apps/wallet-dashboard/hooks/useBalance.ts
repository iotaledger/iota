// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useSuiClient } from '@mysten/dapp-kit';
import { CoinBalance } from '@mysten/sui.js/client';
import { MIST_PER_SUI } from '@mysten/sui.js/utils';
import { useEffect, useMemo, useState } from 'react';

export function useBalance() {
	const account = useCurrentAccount();
	const suiClient = useSuiClient();
	const [coinBalance, setCoinBalance] = useState<CoinBalance>();

	useEffect(() => {
		const fetchBalance = async () => {
			if (account?.address) {
				try {
					const response = await suiClient.getBalance({ owner: account.address });
					setCoinBalance(response);
				} catch (error) {
					console.error('Failed to fetch balance:', error);
				}
			}
		};

		fetchBalance();
	}, [account, suiClient]);

	const calculateBalance = useMemo(() => {
		if (coinBalance) {
			return Number(coinBalance?.totalBalance) / Number(MIST_PER_SUI);
		}
		return 0;
	}, [coinBalance]);

	return {
		coinBalance,
		calculateBalance,
	};
}
