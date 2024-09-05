// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable @typescript-eslint/no-explicit-any */

import {
    DisplayStats,
    DisplayStatsSize,
    DisplayStatsType,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import {
    type ApyByValidator,
    formatAmount,
    formatPercentageDisplay,
    roundFloat,
    useFormatCoin,
    useGetValidatorsApy,
    useGetValidatorsEvents,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaEvent, type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';
import React, { type JSX, useMemo } from 'react';

import { ErrorBoundary, PageLayout, StakeColumn } from '~/components';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Banner, Link, PlaceholderTable, TableCard, TableHeader, Tooltip } from '~/components/ui';
import { ampli, getValidatorMoveEvent } from '~/lib/utils';
import { type ColumnDef } from '@tanstack/react-table';
import { Badge, BadgeType, type TableCellProps, TableCellType } from '@iota/apps-ui-kit';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib';

export interface ValidatorTableRow {
    name: TableCellProps;
    stake: TableCellProps;
    apy:
        | TableCellProps
        | {
              apy: number | null;
              isApyApproxZero?: boolean;
          };
    nextEpochGasPrice: TableCellProps;
    commission: TableCellProps;
    img: string | null;
    address: string;
    lastReward:
        | TableCellProps
        | {
              lastReward: number | null;
          };
    votingPower: TableCellProps;
    atRisk: TableCellProps;
    symbol?: string;
}

interface ValidatorsTableDataArgs {
    validators: IotaValidatorSummary[];
    atRiskValidators: [string, string][];
    validatorEvents: IotaEvent[];
    rollingAverageApys: ApyByValidator | null;
    columns?: ColumnDef<object, unknown>[];
    symbol?: string;
}

function generateValidatorName(address: string, name: string, imageUrl: string) {
    return (
        <Link
            to={`/validator/${encodeURIComponent(address)}`}
            onClick={() =>
                ampli.clickedValidatorRow({
                    sourceFlow: 'Epoch details',
                    validatorAddress: address,
                    validatorName: name,
                })
            }
        >
            <div className="flex items-center gap-x-2.5 text-neutral-40 dark:text-neutral-60">
                <span className="text-label-lg">{name}</span>
            </div>
        </Link>
    );
}

function generateAtRiskValidator(atRisk: number | null) {
    const label = 'At Risk';
    return typeof atRisk === 'number' ? (
        <Tooltip
            tip="Staked IOTA is below the minimum IOTA stake threshold to remain a validator."
            onOpen={() =>
                ampli.activatedTooltip({
                    tooltipLabel: label,
                })
            }
        >
            <div className="flex cursor-pointer flex-nowrap items-center">
                <div className="text-body-md uppercase text-neutral-40">{label}</div>
                &nbsp;
                <div className="text-body-md uppercase text-neutral-40">
                    {atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch'}
                </div>
            </div>
        </Tooltip>
    ) : (
        <Badge type={BadgeType.Neutral} label="Active" />
    );
}

export function validatorsTableData({
    validators,
    atRiskValidators,
    validatorEvents,
    rollingAverageApys,
    columns,
    symbol,
}: ValidatorsTableDataArgs): { data: ValidatorTableRow[]; columns: ColumnDef<object, unknown>[] } {
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
                        type: TableCellType.Children,
                        children: generateValidatorName(
                            validator.iotaAddress,
                            validatorName,
                            validator.imageUrl,
                        ),
                    },
                    stake: {
                        type: TableCellType.Text,
                        label: formatAmount(totalStake),
                        supportingLabel: symbol,
                        stake: totalStake,
                    },
                    apy: {
                        type: TableCellType.AvatarText,
                        label: formatPercentageDisplay(apy, '--', isApyApproxZero),
                        apy,
                        isApyApproxZero,
                    },
                    nextEpochGasPrice: {
                        type: TableCellType.Text,
                        label: formatAmount(validator.nextEpochGasPrice),
                        supportingLabel: symbol,
                        nextEpochGasPrice: validator.nextEpochGasPrice,
                    },
                    commission: {
                        type: TableCellType.Text,
                        label: `${Number(validator.commissionRate) / 100}%`,
                    },
                    img: img,
                    address: validator.iotaAddress,
                    lastReward: {
                        type: TableCellType.Text,
                        label: lastReward ? formatAmount(Number(lastReward)) : '--',
                        supportingLabel: lastReward ? symbol : undefined,
                        lastReward: lastReward ?? null,
                    },
                    votingPower: {
                        type: TableCellType.Text,
                        label: validator.votingPower
                            ? Number(validator.votingPower) / 100 + '%'
                            : '--',
                    },
                    atRisk: {
                        type: TableCellType.Children,
                        children: generateAtRiskValidator(
                            isAtRisk
                                ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1])
                                : null,
                        ),
                    },
                    symbol: symbol,
                };
            }),
        columns: columns || [
            {
                header: '#',
                accessorKey: 'number',
                meta: {
                    isRenderCell: true,
                },
                cell: (props: any) => (
                    <div className="text-body-md text-neutral-40">
                        {props.table.getSortedRowModel().flatRows.indexOf(props.row) + 1}
                    </div>
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
            },
            {
                header: 'Stake',
                accessorKey: 'stake',
                enableSorting: true,
                meta: {
                    isRenderCell: true,
                },
                cell: (props: any) => <StakeColumn stake={props.getValue().stake} />,
            },
            {
                header: 'Proposed Next Epoch Gas Price',
                accessorKey: 'nextEpochGasPrice',
                enableSorting: true,
                meta: {
                    isRenderCell: true,
                },
                cell: (props: any) => (
                    <StakeColumn stake={props.getValue().nextEpochGasPrice} inNano />
                ),
            },
            {
                header: 'APY',
                accessorKey: 'apy',
                enableSorting: true,
                meta: {
                    isRenderCell: true,
                },
                sortingFn: (a: any, b: any, colId: string) =>
                    a.getValue(colId)?.apy < b.getValue(colId)?.apy ? -1 : 1,
                cell: (props: any) => {
                    const { apy, isApyApproxZero } = props.getValue();
                    return (
                        <div className="text-body-md text-neutral-40">
                            {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        </div>
                    );
                },
            },
            {
                header: 'Commission',
                accessorKey: 'commission',
                enableSorting: true,
            },
            {
                header: 'Last Epoch Rewards',
                accessorKey: 'lastReward',
                enableSorting: true,
                cell: (props: any) => {
                    const lastReward = props.getValue().lastReward;
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
            },
            {
                header: 'Status',
                accessorKey: 'atRisk',
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
        return validatorsTableData({
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
