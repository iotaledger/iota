// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef, Row } from '@tanstack/react-table';
import { type ApyByValidator, formatPercentageDisplay, ImageIcon, ImageIconSize } from '@iota/core';
import {
    ampli,
    getValidatorMoveEvent,
    type IotaValidatorSummaryExtended,
    VALIDATOR_LOW_STAKE_GRACE_PERIOD,
} from '~/lib';
import { StakeColumn } from '~/components';
import type { IotaEvent, IotaValidatorSummary } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { ValidatorLink } from '~/components/ui';

interface generateValidatorsTableColumnsArgs {
    atRiskValidators: [string, string][];
    validatorEvents: IotaEvent[];
    rollingAverageApys: ApyByValidator | null;
    limit?: number;
    showValidatorIcon?: boolean;
    includeColumns?: string[];
    highlightValidatorName?: boolean;
}

function ValidatorWithImage({
    validator,
    highlightValidatorName,
}: {
    validator: IotaValidatorSummaryExtended;
    highlightValidatorName?: boolean;
}) {
    return validator.isPending ? (
        <div className="flex items-center gap-x-2.5 text-neutral-40 dark:text-neutral-60">
            <div className="h-8 w-8 shrink-0">
                <ImageIcon
                    src={validator.imageUrl}
                    label={validator.name}
                    fallback={validator.name}
                    size={ImageIconSize.Medium}
                    rounded
                />
            </div>
            <span
                className={clsx('text-label-lg', {
                    'text-neutral-10 dark:text-neutral-92': highlightValidatorName,
                })}
            >
                {validator.name}
            </span>
        </div>
    ) : (
        <ValidatorLink
            address={validator.iotaAddress}
            onClick={() =>
                ampli.clickedValidatorRow({
                    sourceFlow: 'Epoch details',
                    validatorAddress: validator.iotaAddress,
                    validatorName: validator.name,
                })
            }
            label={
                <div className="flex items-center gap-x-2.5 text-neutral-40 dark:text-neutral-60">
                    <div className="h-8 w-8 shrink-0">
                        <ImageIcon
                            src={validator.imageUrl}
                            label={validator.name}
                            fallback={validator.name}
                            size={ImageIconSize.Medium}
                            rounded
                        />
                    </div>
                    <span
                        className={clsx('text-label-lg', {
                            'text-neutral-10 dark:text-neutral-92': highlightValidatorName,
                        })}
                    >
                        {validator.name}
                    </span>
                </div>
            }
        />
    );
}

export function generateValidatorsTableColumns({
    atRiskValidators = [],
    validatorEvents = [],
    rollingAverageApys = null,
    showValidatorIcon = true,
    includeColumns,
    highlightValidatorName,
}: generateValidatorsTableColumnsArgs): ColumnDef<IotaValidatorSummaryExtended>[] {
    let columns: ColumnDef<IotaValidatorSummaryExtended>[] = [
        {
            header: '#',
            id: 'number',
            cell({ row }) {
                return (
                    <TableCellBase>
                        <TableCellText>{row.index + 1}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Name',
            id: 'name',
            accessorKey: 'name',
            enableSorting: true,
            sortingFn: (row1, row2, columnId) => {
                const value1 = row1.getValue<string>(columnId);
                const value2 = row2.getValue<string>(columnId);
                return sortByString(value1, value2);
            },
            cell({ row: { original: validator } }) {
                return (
                    <TableCellBase>
                        {showValidatorIcon ? (
                            <ValidatorWithImage
                                validator={validator}
                                highlightValidatorName={highlightValidatorName}
                            />
                        ) : (
                            <TableCellText>
                                <span
                                    className={
                                        highlightValidatorName
                                            ? 'text-neutral-10 dark:text-neutral-92'
                                            : undefined
                                    }
                                >
                                    {validator.name}
                                </span>
                            </TableCellText>
                        )}
                    </TableCellBase>
                );
            },
        },

        {
            header: 'Stake',
            accessorKey: 'stakingPoolIotaBalance',
            enableSorting: true,
            sortingFn: (rowA, rowB, columnId) =>
                BigInt(rowA.getValue(columnId)) - BigInt(rowB.getValue(columnId)) > 0 ? 1 : -1,
            cell({ getValue }) {
                const stakingPoolIotaBalance = getValue<string>();
                return (
                    <TableCellBase>
                        <StakeColumn stake={stakingPoolIotaBalance} />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Proposed next Epoch gas price',
            accessorKey: 'nextEpochGasPrice',
            cell({ getValue }) {
                const nextEpochGasPrice = getValue<string>();
                return (
                    <TableCellBase>
                        <StakeColumn stake={nextEpochGasPrice} inNano />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'APY',
            accessorKey: 'iotaAddress',
            enableSorting: true,
            sortingFn: (rowA, rowB, columnId) => {
                const apyA = rollingAverageApys?.[rowA.getValue<string>(columnId)]?.apy ?? null;
                const apyB = rollingAverageApys?.[rowB.getValue<string>(columnId)]?.apy ?? null;

                // Handle null values: move nulls to the bottom
                if (apyA === null) return 1;
                if (apyB === null) return -1;

                return apyA - apyB;
            },
            cell({ getValue }) {
                const iotaAddress = getValue<string>();
                const { apy, isApyApproxZero } = rollingAverageApys?.[iotaAddress] ?? {
                    apy: null,
                };
                return (
                    <TableCellBase>
                        <TableCellText>
                            {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Commission',
            accessorKey: 'commissionRate',
            enableSorting: true,
            sortingFn: sortByNumber,
            cell({ getValue }) {
                return (
                    <TableCellBase>
                        <TableCellText>{`${Number(getValue()) / 100}%`}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Last Epoch Rewards',
            accessorKey: 'lastReward',
            id: 'lastReward',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const lastRewardA = getLastReward(validatorEvents, rowA);
                const lastRewardB = getLastReward(validatorEvents, rowB);

                if (lastRewardA === null) return 1;
                if (lastRewardB === null) return -1;

                return lastRewardA > lastRewardB ? 1 : -1;
            },
            cell({ row }) {
                const lastReward = getLastReward(validatorEvents, row);
                return (
                    <TableCellBase>
                        <TableCellText>
                            {lastReward !== null ? <StakeColumn stake={lastReward} /> : '--'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Voting Power',
            accessorKey: 'votingPower',
            enableSorting: true,
            sortingFn: sortByNumber,
            cell({ getValue }) {
                const votingPower = getValue<string>();
                return (
                    <TableCellBase>
                        <TableCellText>
                            {votingPower ? Number(votingPower) / 100 + '%' : '--'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },

        {
            header: 'Status',
            accessorKey: 'atRisk',
            id: 'atRisk',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const { label: labelA } = determineRisk(atRiskValidators, rowA);
                const { label: labelB } = determineRisk(atRiskValidators, rowB);
                return sortByString(labelA, labelB);
            },
            cell({ row }) {
                const { atRisk, label, isPending } = determineRisk(atRiskValidators, row);

                if (isPending) {
                    return (
                        <TableCellBase>
                            <Badge type={BadgeType.Neutral} label={label} />
                        </TableCellBase>
                    );
                }
                return (
                    <TableCellBase>
                        <Badge
                            type={atRisk === null ? BadgeType.PrimarySoft : BadgeType.Neutral}
                            label={label}
                        />
                    </TableCellBase>
                );
            },
        },
    ];

    if (includeColumns) {
        columns = columns.filter((col) =>
            includeColumns.includes(col.header?.toString() as string),
        );
    }

    return columns;
}
function sortByString(value1: string, value2: string) {
    return value1.localeCompare(value2, undefined, { sensitivity: 'base' });
}
function sortByNumber(
    rowA: Row<IotaValidatorSummary>,
    rowB: Row<IotaValidatorSummary>,
    columnId: string,
) {
    return Number(rowA.getValue(columnId)) - Number(rowB.getValue(columnId)) > 0 ? 1 : -1;
}
function getLastReward(
    validatorEvents: IotaEvent[],
    row: Row<IotaValidatorSummaryExtended>,
): number | null {
    const { original: validator } = row;
    const event = getValidatorMoveEvent(validatorEvents, validator.iotaAddress) as {
        pool_staking_reward?: string;
    };
    return event?.pool_staking_reward ? Number(event.pool_staking_reward) : null;
}
function determineRisk(
    atRiskValidators: [string, string][],
    row: Row<IotaValidatorSummaryExtended>,
) {
    const { original: validator } = row;
    const atRiskValidator = atRiskValidators.find(([address]) => address === validator.iotaAddress);
    const isAtRisk = !!atRiskValidator;
    const atRisk = isAtRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1]) : null;
    const isPending = validator.isPending;
    const label = isPending
        ? 'Pending'
        : atRisk === null
          ? 'Active'
          : atRisk > 1
            ? `At Risk in ${atRisk} epochs`
            : 'At Risk next epoch';
    return {
        label,
        atRisk,
        isPending,
    };
}
