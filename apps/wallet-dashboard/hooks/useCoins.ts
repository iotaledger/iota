// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useSuiClient } from '@mysten/dapp-kit';
import { PaginatedCoins } from '@mysten/sui.js/client';
import { useEffect, useState } from 'react';

export function useCoins() {
	const account = useCurrentAccount();
	const suiClient = useSuiClient();
	const [allCoins, setAllCoins] = useState<PaginatedCoins>();

	useEffect(() => {
		const fetchAllCoins = async () => {
			if (account?.address) {
				try {
					const response = await suiClient.getAllCoins({ owner: account.address });
					setAllCoins(response);
				} catch (error) {
					console.error('Failed to fetch all coins:', error);
				}
			}
		};

		fetchAllCoins();
	}, [account, suiClient]);

	return {
		coins: allCoins,
	};
}
