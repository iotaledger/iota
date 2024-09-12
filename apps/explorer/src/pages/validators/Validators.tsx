// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable @typescript-eslint/no-explicit-any */

import { DisplayStats, DisplayStatsSize, DisplayStatsType, TooltipPosition } from '@iota/apps-ui-kit';
import {
    formatPercentageDisplay,
    roundFloat,
    useFormatCoin,
    useGetValidatorsApy,
    useGetValidatorsEvents,
    type ApyByValidator,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaEvent, type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Text } from '@iota/ui';
import { useMemo } from 'react';
import { t } from 'vitest/dist/types-198fd1d9';
import { ErrorBoundary, PageLayout, StakeColumn } from '~/components';
import {
    Banner,
    ImageIcon,
    Link,
    PlaceholderTable,
    TableCard,
    TableHeader,
    Tooltip,
} from '~/components/ui';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { ampli, getValidatorMoveEvent } from '~/lib/utils';

export function validatorsTableData(
    validators: IotaValidatorSummary[],
    atRiskValidators: [string, string][],
    validatorEvents: IotaEvent[],
    rollingAverageApys: ApyByValidator | null,
) {
    return {
        data: [...validators]
            .sort(() => 0.5 - Math.random())
            .map((validator) => {
                const validatorName = validator.name;
                const totalStake = validator.stakingPoolIotaBalance;
                const img = validator.imageUrl;

                const event = getValidatorMoveEvent(validatorEvents, validator.iotaAddress) as {
                    pool_staking_reward?: string;
                };

                const atRiskValidator = atRiskValidators.find(
                    ([address]) => address === validator.iotaAddress,
                );
                const isAtRisk = !!atRiskValidator;
                const lastReward = event?.pool_staking_reward ?? null;
                const { apy, isApyApproxZero } = rollingAverageApys?.[validator.iotaAddress] ?? {
                    apy: null,
                };

                return {
                    name: {
                        name: validatorName,
                        logo: validator.imageUrl,
                    },
                    stake: totalStake,
                    apy: {
                        apy,
                        isApyApproxZero,
                    },
                    nextEpochGasPrice: validator.nextEpochGasPrice,
                    commission: Number(validator.commissionRate) / 100,
                    img: img,
                    address: validator.iotaAddress,
                    lastReward: lastReward ?? null,
                    votingPower: Number(validator.votingPower) / 100,
                    atRisk: isAtRisk
                        ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1])
                        : null,
                };
            }),
        columns: [
            {
                header: '#',
                accessorKey: 'number',
                cell: (props: any) => (
                    <Text variant="bodySmall/medium" color="steel-dark">
                        {props.table.getSortedRowModel().flatRows.indexOf(props.row) + 1}
                    </Text>
                ),
            },
            {
                header: 'Name',
                accessorKey: 'name',
                enableSorting: true,
                sortingFn: (a: any, b: any, colId: string) =>
                    a.getValue(colId).name.localeCompare(b.getValue(colId).name, 'en', {
                        sensitivity: 'base',
                        numeric: true,
                    }),
                cell: (props: any) => {
                    const { name, logo } = props.getValue();
                    return (
                        <Link
                            to={`/validator/${encodeURIComponent(props.row.original.address)}`}
                            onClick={() =>
                                ampli.clickedValidatorRow({
                                    sourceFlow: 'Epoch details',
                                    validatorAddress: props.row.original.address,
                                    validatorName: name,
                                })
                            }
                        >
                            <div className="flex items-center gap-2.5">
                                <ImageIcon
                                    src={logo}
                                    size="sm"
                                    label={name}
                                    fallback={name}
                                    circle
                                />
                                <Text variant="bodySmall/medium" color="steel-darker">
                                    {name}
                                </Text>
                            </div>
                        </Link>
                    );
                },
            },
            {
                header: 'Stake',
                accessorKey: 'stake',
                enableSorting: true,
                cell: (props: any) => <StakeColumn stake={props.getValue()} />,
            },
            {
                header: 'Proposed Next Epoch Gas Price',
                accessorKey: 'nextEpochGasPrice',
                enableSorting: true,
                cell: (props: any) => <StakeColumn stake={props.getValue()} inNano />,
            },
            {
                header: 'APY',
                accessorKey: 'apy',
                enableSorting: true,
                sortingFn: (a: any, b: any, colId: string) =>
                    a.getValue(colId)?.apy < b.getValue(colId)?.apy ? -1 : 1,
                cell: (props: any) => {
                    const { apy, isApyApproxZero } = props.getValue();
                    return (
                        <Text variant="bodySmall/medium" color="steel-darker">
                            {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        </Text>
                    );
                },
            },
            {
                header: 'Commission',
                accessorKey: 'commission',
                enableSorting: true,
                cell: (props: any) => {
                    const commissionRate = props.getValue();
                    return (
                        <Text variant="bodySmall/medium" color="steel-darker">
                            {commissionRate}%
                        </Text>
                    );
                },
            },
            {
                header: 'Last Epoch Rewards',
                accessorKey: 'lastReward',
                enableSorting: true,
                cell: (props: any) => {
                    const lastReward = props.getValue();
                    return lastReward !== null ? (
                        <StakeColumn stake={Number(lastReward)} />
                    ) : (
                        <Text variant="bodySmall/medium" color="steel-darker">
                            --
                        </Text>
                    );
                },
            },
            {
                header: 'Voting Power',
                accessorKey: 'votingPower',
                enableSorting: true,
                cell: (props: any) => {
                    const votingPower = props.getValue();
                    return (
                        <Text variant="bodySmall/medium" color="steel-darker">
                            {votingPower}%
                        </Text>
                    );
                },
            },
            {
                header: 'Status',
                accessorKey: 'atRisk',
                cell: (props: any) => {
                    const atRisk = props.getValue();
                    const label = 'At Risk';
                    return atRisk !== null ? (
                        <Tooltip
                            tip="Staked IOTA is below the minimum IOTA stake threshold to remain a validator."
                            onOpen={() =>
                                ampli.activatedTooltip({
                                    tooltipLabel: label,
                                })
                            }
                        >
                            <div className="flex cursor-pointer flex-nowrap items-center">
                                <Text color="issue" variant="bodySmall/medium">
                                    {label}
                                </Text>
                                &nbsp;
                                <Text uppercase variant="bodySmall/medium" color="steel-dark">
                                    {atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch'}
                                </Text>
                            </div>
                        </Tooltip>
                    ) : (
                        <Text variant="bodySmall/medium" color="steel-darker">
                            Active
                        </Text>
                    );
                },
            },
        ],
    };
}

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
        return validatorsTableData(
            data.activeValidators,
            data.atRiskValidators,
            validatorEvents,
            validatorsApy || null,
        );
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
                                        sortTable
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
