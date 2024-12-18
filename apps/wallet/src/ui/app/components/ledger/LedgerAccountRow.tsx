// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useResolveIotaNSName } from '_app/hooks/useAppResolveIotaNSName';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { CheckFill16 } from '@iota/icons';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import cl from 'clsx';

import { useCoinsReFetchingConfig } from '../../hooks';

type LedgerAccountRowProps = {
	isSelected: boolean;
	address: string;
};

export function LedgerAccountRow({ isSelected, address }: LedgerAccountRowProps) {
	const { staleTime, refetchInterval } = useCoinsReFetchingConfig();

	const { data: coinBalance } = useIotaClientQuery(
		'getBalance',
		{
			coinType: IOTA_TYPE_ARG,
			owner: address,
		},
		{
			refetchInterval,
			staleTime,
		},
	);
	const domainName = useResolveIotaNSName(address);

	const [totalAmount, totalAmountSymbol] = useFormatCoin(
		coinBalance?.totalBalance ?? 0,
		IOTA_TYPE_ARG,
	);

	return (
		<div className="flex items-center gap-3">
			<CheckFill16
				className={cl('w-4 h-4', {
					'text-gray-50': !isSelected,
					'text-success': isSelected,
				})}
			/>
			<Text
				mono
				variant="bodySmall"
				weight="semibold"
				color={isSelected ? 'steel-darker' : 'steel-dark'}
			>
				{domainName ?? formatAddress(address)}
			</Text>
			<div className="ml-auto">
				<Text variant="bodySmall" color="steel" weight="semibold" mono>
					{totalAmount} {totalAmountSymbol}
				</Text>
			</div>
		</div>
	);
}
