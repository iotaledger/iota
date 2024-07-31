// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { IconTooltip } from '_app/shared/tooltip';
import LoadingIndicator from '_components/loading/LoadingIndicator';
import { roundFloat, useGetValidatorsApy } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';

const APY_DECIMALS = 3;

interface DelegatedAPYProps {
    stakedValidators: string[];
}

export function DelegatedAPY({ stakedValidators }: DelegatedAPYProps) {
    const { data, isPending } = useIotaClientQuery('getLatestIotaSystemState');
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
                    <div className="text-steel flex items-baseline text-body">
                        <IconTooltip
                            tip="The average APY of all validators you are currently staking your IOTA on."
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
