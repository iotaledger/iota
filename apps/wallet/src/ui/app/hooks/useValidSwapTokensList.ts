// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { useGetAllBalances } from '_app/hooks/useGetAllBalances';
import { useRecognizedPackages } from '_app/hooks/useRecognizedPackages';
import { useSupportedCoins } from '_app/hooks/useSupportedCoins';
import { type CoinBalance } from '@iota/iota-sdk/client';
import {
	normalizeStructTag,
	normalizeIotaObjectId,
	parseStructTag,
	IOTA_TYPE_ARG,
} from '@iota/iota-sdk/utils';
import { useMemo } from 'react';

export function filterTokenBalances(tokens: CoinBalance[]) {
	return tokens.filter(
		(token) => Number(token.totalBalance) > 0 || token.coinType === IOTA_TYPE_ARG,
	);
}

export function useValidSwapTokensList() {
	const address = useActiveAddress();
	const { data, isLoading: isSupportedCoinsLoading } = useSupportedCoins();
	const { data: rawCoinBalances, isLoading: isGetAllBalancesLoading } = useGetAllBalances(
		address || '',
	);
	const packages = useRecognizedPackages();
	const normalizedPackages = useMemo(
		() => packages.map((id) => normalizeIotaObjectId(id)),
		[packages],
	);

	const supported = useMemo(
		() =>
			data?.supported.filter((type) => normalizedPackages.includes(parseStructTag(type).address)),
		[data, normalizedPackages],
	);

	const coinBalances = useMemo(
		() => (rawCoinBalances ? filterTokenBalances(rawCoinBalances) : null),
		[rawCoinBalances],
	);

	const validSwaps = useMemo(
		() =>
			supported?.sort((a, b) => {
				const iotaType = normalizeStructTag(IOTA_TYPE_ARG);
				const balanceA = BigInt(
					coinBalances?.find(
						(balance) => normalizeStructTag(balance.coinType) === normalizeStructTag(a),
					)?.totalBalance ?? 0,
				);
				const balanceB = BigInt(
					coinBalances?.find(
						(balance) => normalizeStructTag(balance.coinType) === normalizeStructTag(b),
					)?.totalBalance ?? 0,
				);
				return a === iotaType ? -1 : b === iotaType ? 1 : Number(balanceB - balanceA);
			}) ?? [],
		[supported, coinBalances],
	);

	return {
		isLoading: isSupportedCoinsLoading || isGetAllBalancesLoading,
		data: validSwaps,
	};
}
