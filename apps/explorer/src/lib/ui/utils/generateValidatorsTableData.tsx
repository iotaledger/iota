// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import {
    BadgeType,
    type TableCellProps,
    TableCellType,
    TableCellTextColor,
} from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { type ApyByValidator, formatPercentageDisplay } from '@iota/core';

import { ampli, getValidatorMoveEvent, VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib';
import { Link, StakeColumn, AddressLink, ImageIcon } from '~/components';
import type { IotaEvent, IotaValidatorSummary } from '@iota/iota-sdk/dist/cjs/client';

export interface ValidatorTableRow {
    name: TableCellProps;
    stake: TableCellProps;
    apy: TableCellProps;
    nextEpochGasPrice: TableCellProps;
    commission: TableCellProps;
    address: TableCellProps;
    lastReward: TableCellProps;
    votingPower: TableCellProps;
    atRisk: TableCellProps;
}

interface GenerateValidatorsTableDataArgs {
    validators: IotaValidatorSummary[];
    atRiskValidators: [string, string][];
    validatorEvents: IotaEvent[];
    rollingAverageApys: ApyByValidator | null;
    limit?: number;
    showValidatorIcon?: boolean;
    columns?: ColumnDef<object, unknown>[];
}

const DEFAULT_COLUMNS: ColumnDef<object, unknown>[] = [
    {
        header: '#',
        accessorKey: 'number',
    },
    {
        header: 'Name',
        accessorKey: 'name',
    },
    {
        header: 'Stake',
        accessorKey: 'stake',
    },
    {
        header: 'Proposed Next Epoch Gas Price',
        accessorKey: 'nextEpochGasPrice',
    },
    {
        header: 'APY',
        accessorKey: 'apy',
    },
    {
        header: 'Commission',
        accessorKey: 'commission',
    },
    {
        header: 'Last Epoch Rewards',
        accessorKey: 'lastReward',
    },
    {
        header: 'Voting Power',
        accessorKey: 'votingPower',
    },
    {
        header: 'Status',
        accessorKey: 'atRisk',
    },
];

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
                <ImageIcon src={imageUrl} size="sm" label={name} fallback={name} />
                <span className="text-label-lg">{name}</span>
            </div>
        </Link>
    );
}

function generateValidatorAtRisk(atRisk: number | null): TableCellProps {
    if (atRisk === null) {
        return {
            type: TableCellType.Badge,
            badgeType: BadgeType.Neutral,
            label: 'Active',
        };
    }

    const atRiskText = atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch';
    return {
        type: TableCellType.Badge,
        badgeType: BadgeType.PrimarySolid,
        label: `At Risk ${atRiskText}`,
    };
}

function generateValidatorAddress(address: string, name: string, limit?: number) {
    return (
        <div className="whitespace-nowrap">
            <AddressLink
                address={address}
                noTruncate={!limit}
                onClick={() =>
                    ampli.clickedValidatorRow({
                        sourceFlow: 'Top validators - validator address',
                        validatorAddress: address,
                        validatorName: name,
                    })
                }
            />
        </div>
    );
}

export function generateValidatorsTableData({
    validators,
    limit,
    atRiskValidators = [],
    validatorEvents = [],
    rollingAverageApys = null,
    showValidatorIcon = true,
    columns = DEFAULT_COLUMNS,
}: GenerateValidatorsTableDataArgs): {
    data: ValidatorTableRow[];
    columns: ColumnDef<object, unknown>[];
} {
    return {
        data: [...validators]
            .sort(() => 0.5 - Math.random())
            .map((validator, i) => {
                const validatorName = validator.name;
                const totalStake = validator.stakingPoolIotaBalance;

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
                    number: {
                        type: TableCellType.Text,
                        label: `${i + 1}`,
                        textColor: TableCellTextColor.Dark,
                    },
                    name: showValidatorIcon
                        ? {
                              type: TableCellType.Children,
                              children: generateValidatorName(
                                  validator.iotaAddress,
                                  validatorName,
                                  validator.imageUrl,
                              ),
                          }
                        : {
                              type: TableCellType.Text,
                              label: validatorName,
                              textColor: TableCellTextColor.Dark,
                          },
                    stake: {
                        type: TableCellType.Children,
                        children: <StakeColumn stake={totalStake} />,
                        noWrap: true,
                    },
                    nextEpochGasPrice: {
                        type: TableCellType.Children,
                        children: <StakeColumn stake={validator.nextEpochGasPrice} inNano />,
                        noWrap: true,
                    },
                    apy: {
                        type: TableCellType.Text,
                        label: formatPercentageDisplay(apy, '--', isApyApproxZero),
                    },
                    commission: {
                        type: TableCellType.Text,
                        label: `${Number(validator.commissionRate) / 100}%`,
                    },
                    lastReward:
                        lastReward !== null
                            ? {
                                  type: TableCellType.Children,
                                  children: <StakeColumn stake={Number(lastReward)} />,
                                  noWrap: true,
                              }
                            : {
                                  type: TableCellType.Text,
                                  label: '--',
                              },
                    votingPower: {
                        type: TableCellType.Text,
                        label: validator.votingPower
                            ? Number(validator.votingPower) / 100 + '%'
                            : '--',
                    },
                    atRisk: generateValidatorAtRisk(
                        isAtRisk
                            ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1])
                            : null,
                    ),
                    address: {
                        type: TableCellType.Children,
                        children: generateValidatorAddress(
                            validator.iotaAddress,
                            validatorName,
                            limit,
                        ),
                    },
                };
            }),
        columns: columns,
    };
}
