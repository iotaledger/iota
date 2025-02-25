// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { type ApyByValidator, formatPercentageDisplay, ImageIcon, ImageIconSize } from '@iota/core';
import { ampli, getValidatorMoveEvent, VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib';
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
    validator: IotaValidatorSummary;
    highlightValidatorName?: boolean;
}) {
    return (
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
}: generateValidatorsTableColumnsArgs): ColumnDef<IotaValidatorSummary>[] {
    let columns: ColumnDef<IotaValidatorSummary>[] = [
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
            id: 'lastReward',
            cell({ row: { original: validator } }) {
                const event = getValidatorMoveEvent(validatorEvents, validator.iotaAddress) as {
                    pool_staking_reward?: string;
                };
                const lastReward = event?.pool_staking_reward ?? null;
                return (
                    <TableCellBase>
                        <TableCellText>
                            {lastReward !== null ? (
                                <StakeColumn stake={Number(lastReward)} />
                            ) : (
                                '--'
                            )}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Voting Power',
            accessorKey: 'votingPower',
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
            id: 'atRisk',
            cell({ row: { original: validator } }) {
                const atRiskValidator = atRiskValidators.find(
                    ([address]) => address === validator.iotaAddress,
                );
                const isAtRisk = !!atRiskValidator;
                const atRisk = isAtRisk
                    ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1])
                    : null;

                if (atRisk === null) {
                    return (
                        <TableCellBase>
                            <Badge type={BadgeType.PrimarySoft} label="Active" />
                        </TableCellBase>
                    );
                }

                const atRiskText = atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch';
                return (
                    <TableCellBase>
                        <Badge type={BadgeType.Neutral} label={`At Risk ${atRiskText}`} />
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
