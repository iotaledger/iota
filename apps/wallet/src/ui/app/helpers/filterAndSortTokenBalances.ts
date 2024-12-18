// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { USDC_TYPE_ARG } from '_pages/swap/utils';
import { type CoinBalance } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

// Sort tokens by symbol and total balance
// Move this to the API backend
// Filter out tokens with zero balance
export function filterAndSortTokenBalances(tokens: CoinBalance[]) {
	return tokens
		.filter((token) => Number(token.totalBalance) > 0)
		.sort((a, b) => {
			if (a.coinType === IOTA_TYPE_ARG) {
				return -1;
			}
			if (b.coinType === IOTA_TYPE_ARG) {
				return 1;
			}
			if (a.coinType === USDC_TYPE_ARG) {
				return -1;
			}
			if (b.coinType === USDC_TYPE_ARG) {
				return 1;
			}
			return (getCoinSymbol(a.coinType) + Number(a.totalBalance)).localeCompare(
				getCoinSymbol(b.coinType) + Number(b.totalBalance),
			);
		});
}

export function getCoinSymbol(coinTypeArg: string) {
	return coinTypeArg.substring(coinTypeArg.lastIndexOf(':') + 1);
}
