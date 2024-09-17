// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { type ApyByValidator, formatPercentageDisplay } from '@iota/core';

import { ampli, getValidatorMoveEvent, VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib';
import { StakeColumn, ValidatorLink, ImageIcon } from '~/components';
import type { IotaEvent, IotaValidatorSummary } from '@iota/iota-sdk/dist/cjs/client';

interface generateValidatorsTableColumnsArgs {
    atRiskValidators: [string, string][];
    validatorEvents: IotaEvent[];
    rollingAverageApys: ApyByValidator | null;
    limit?: number;
    showValidatorIcon?: boolean;
    filterColumns?: string[];
}

function ValidatorWithImage({ validator }: { validator: IotaValidatorSummary }) {
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
        >
            <div className="flex items-center gap-x-2.5 text-neutral-40 dark:text-neutral-60">
                <ImageIcon
                    src={validator.imageUrl}
                    size="sm"
                    label={validator.name}
                    fallback={validator.name}
                />
                <span className="text-label-lg">{validator.name}</span>
            </div>
        </ValidatorLink>
    );
}

function ValidatorAddress({
    validator,
    limit,
}: {
    validator: IotaValidatorSummary;
    limit?: number;
}) {
    return (
        <div className="whitespace-nowrap">
            <ValidatorLink
                address={validator.iotaAddress}
                noTruncate={!limit}
                onClick={() =>
                    ampli.clickedValidatorRow({
                        sourceFlow: 'Top validators - validator address',
                        validatorAddress: validator.iotaAddress,
                        validatorName: validator.name,
                    })
                }
            />
        </div>
    );
}

export function generateValidatorsTableColumns({
    limit,
    atRiskValidators = [],
    validatorEvents = [],
    rollingAverageApys = null,
    showValidatorIcon = true,
    filterColumns,
}: generateValidatorsTableColumnsArgs): ColumnDef<IotaValidatorSummary>[] {
    let columns: ColumnDef<IotaValidatorSummary>[] = [
        {
            header: 'Name',
            id: 'name',
            cell({ row: { original: validator } }) {
                return (
                    <TableCellBase>
                        {showValidatorIcon ? (
                            <ValidatorWithImage validator={validator} />
                        ) : (
                            <TableCellText>{validator.name}</TableCellText>
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
            header: 'nextEpochGasPrice',
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
            header: 'apy',
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
            header: 'Comission',
            accessorKey: 'comission',
            cell({ getValue }) {
                return (
                    <TableCellBase>
                        <TableCellText>{`${Number(getValue()) / 100}%`}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Last Reward',
            id: 'lastReward',
            cell({ row: { original: validator } }) {
                const event = getValidatorMoveEvent(validatorEvents, validator.iotaAddress) as {
                    pool_staking_reward?: string;
                };
                const lastReward = event?.pool_staking_reward ?? null;
                if (lastReward !== null) {
                    return <StakeColumn stake={Number(lastReward)} />;
                } else {
                    <TableCellText>--</TableCellText>;
                }
            },
        },
        {
            header: 'Voting Power',
            accessorKey: 'votingPower',
            cell({ getValue }) {
                const votingPower = getValue<string>();
                return (
                    <TableCellText>
                        {votingPower ? Number(votingPower) / 100 + '%' : '--'}
                    </TableCellText>
                );
            },
        },

        {
            header: 'At Risk',
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
                    return <Badge type={BadgeType.PrimarySoft} label="Active" />;
                }

                const atRiskText = atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch';
                return <Badge type={BadgeType.Neutral} label={`At Risk ${atRiskText}`} />;
            },
        },
        {
            header: 'Address',
            id: 'address',
            cell({ row: { original: validator } }) {
                return (
                    <TableCellBase>
                        <TableCellText>
                            <ValidatorAddress validator={validator} limit={limit} />
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];

    if (filterColumns) {
        columns = columns.filter((col) => filterColumns.includes(col.header?.toString() as string));
    }

    return columns;
}
