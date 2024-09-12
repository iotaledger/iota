// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { type JSX, useMemo } from 'react';
import { roundFloat, useFormatCoin, useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import {
    DisplayStats,
    DisplayStatsSize,
    DisplayStatsType,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    ErrorBoundary,
    PageLayout,
    Banner,
    PlaceholderTable,
    TableCard,
    TableHeader,
} from '~/components';
import { generateValidatorsTableData } from '~/lib/ui/utils/generateValidatorsTableData';

function ValidatorPageResult(): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');
    const numberOfValidators = data?.activeValidators.length || 0;

    const {
        data: validatorEvents,
        isPending: validatorsEventsLoading,
        isError: validatorEventError,
    } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });

    const { data: validatorsApy } = useGetValidatorsApy();

    const totalStaked = useMemo(() => {
        if (!data) return 0;
        const validators = data.activeValidators;

        return validators.reduce((acc, cur) => acc + Number(cur.stakingPoolIotaBalance), 0);
    }, [data]);

    const averageAPY = useMemo(() => {
        if (!validatorsApy || Object.keys(validatorsApy)?.length === 0) return null;

        // if all validators have isApyApproxZero, return ~0
        if (Object.values(validatorsApy)?.every(({ isApyApproxZero }) => isApyApproxZero)) {
            return '~0';
        }

        // exclude validators with no apy
        const apys = Object.values(validatorsApy)?.filter((a) => a.apy > 0 && !a.isApyApproxZero);
        const averageAPY = apys?.reduce((acc, cur) => acc + cur.apy, 0);
        // in case of no apy, return 0
        return apys.length > 0 ? roundFloat(averageAPY / apys.length) : 0;
    }, [validatorsApy]);

    const lastEpochRewardOnAllValidators = useMemo(() => {
        if (!validatorEvents) return null;
        let totalRewards = 0;

        validatorEvents.forEach(({ parsedJson }) => {
            totalRewards += Number(
                (parsedJson as { pool_staking_reward: string }).pool_staking_reward,
            );
        });

        return totalRewards;
    }, [validatorEvents]);

    const validatorsTable = useMemo(() => {
        if (!data || !validatorEvents) return null;
        return generateValidatorsTableData({
            validators: data.activeValidators,
            atRiskValidators: data.atRiskValidators,
            validatorEvents,
            rollingAverageApys: validatorsApy || null,
        });
    }, [data, validatorEvents, validatorsApy]);

    const [formattedTotalStakedAmount, totalStakedSymbol] = useFormatCoin(
        totalStaked,
        IOTA_TYPE_ARG,
    );
    const [formattedlastEpochRewardOnAllValidatorsAmount, lastEpochRewardOnAllValidatorsSymbol] =
        useFormatCoin(lastEpochRewardOnAllValidators, IOTA_TYPE_ARG);

    const validatorStats = [
        {
            title: 'Total Staked',
            value: formattedTotalStakedAmount,
            supportingLabel: totalStakedSymbol,
            tooltipText:
                'The combined IOTA staked by validators and delegators on the network to support validation and generate rewards.',
        },
        {
            title: 'Participation',
            value: '--',
            tooltipText: 'Coming soon',
        },
        {
            title: 'Last Epoch Rewards',
            value: formattedlastEpochRewardOnAllValidatorsAmount,
            supportingLabel: lastEpochRewardOnAllValidatorsSymbol,
            tooltipText: 'The staking rewards earned in the previous epoch.',
        },
        {
            title: 'AVG APY',
            value: averageAPY ? `${averageAPY}%` : '--',
            tooltipText:
                'The average annualized percentage yield globally for all involved validators.',
        },
    ];

    return (
        <PageLayout
            content={
                isError || validatorEventError ? (
                    <Banner variant="error" fullWidth>
                        Validator data could not be loaded
                    </Banner>
                ) : (
                    <div className="flex w-full flex-col gap-xl">
                        <div className="py-md--rs text-display-sm">Validators</div>
                        <div className="flex w-full flex-col gap-md--rs md:h-40 md:flex-row">
                            {validatorStats.map((stat) => (
                                <DisplayStats
                                    key={stat.title}
                                    label={stat.title}
                                    tooltipText={stat.tooltipText}
                                    value={stat.value}
                                    supportingLabel={stat.supportingLabel}
                                    type={DisplayStatsType.Secondary}
                                    size={DisplayStatsSize.Large}
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            ))}
                        </div>
                        <div>
                            <ErrorBoundary>
                                <TableHeader>All Validators</TableHeader>
                                {(isPending || validatorsEventsLoading) && (
                                    <PlaceholderTable
                                        rowCount={20}
                                        rowHeight="13px"
                                        colHeadings={['Name', 'Address', 'Stake']}
                                    />
                                )}

                                {isSuccess && validatorsTable?.data && (
                                    <TableCard
                                        data={validatorsTable.data}
                                        columns={validatorsTable.columns}
                                        areHeadersCentered={false}
                                    />
                                )}
                            </ErrorBoundary>
                        </div>
                    </div>
                )
            }
        />
    );
}

export { ValidatorPageResult };
