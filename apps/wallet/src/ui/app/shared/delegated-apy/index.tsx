// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { IconTooltip } from '_app/shared/tooltip';
import LoadingIndicator from '_components/loading/LoadingIndicator';
import { roundFloat, useGetValidatorsApy } from '@mysten/core';
import { useSuiClientQuery } from '@mysten/dapp-kit';
import { useMemo } from 'react';

const APY_DECIMALS = 3;

type DelegatedAPYProps = {
	stakedValidators: string[];
};

export function DelegatedAPY({ stakedValidators }: DelegatedAPYProps) {
	const { data, isPending } = useSuiClientQuery('getLatestSuiSystemState');
	const { data: rollingAverageApys } = useGetValidatorsApy();

	const averageNetworkAPY = useMemo(() => {
		if (!data || !rollingAverageApys) return null;

		let stakedAPYs = 0;

		stakedValidators.forEach((validatorAddress) => {
			stakedAPYs += rollingAverageApys?.[validatorAddress]?.apy || 0;
		});

		const averageAPY = stakedAPYs / stakedValidators.length;

		return roundFloat(averageAPY || 0, APY_DECIMALS);
	}, [data, rollingAverageApys, stakedValidators]);

	if (isPending) {
		return (
			<div className="flex h-full w-full items-center justify-center p-2">
				<LoadingIndicator />
			</div>
		);
	}

	if (!averageNetworkAPY) return null;

	return (
		<div className="flex items-center gap-0.5">
			{averageNetworkAPY !== null ? (
				<>
					<Text variant="body" weight="semibold" color="steel-dark">
						{averageNetworkAPY}
					</Text>
					<Text variant="subtitle" weight="medium" color="steel-darker">
						% APY
					</Text>
					<div className="flex items-baseline text-body text-steel">
						<IconTooltip
							tip="The average APY of all validators you are currently staking your SUI on."
							placement="top"
						/>
					</div>
				</>
			) : (
				<Text variant="subtitle" weight="medium" color="steel-dark">
					--
				</Text>
			)}
		</div>
	);
}
