// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BadgeType, type TableCell, TableCellType, TableCellTextColor } from '@iota/apps-ui-kit';
import { formatPercentageDisplay, type ApyByValidator } from '@iota/core';
import { type IotaEvent, type IotaValidatorSummary } from '@iota/iota-sdk/client';
import React, { type ComponentProps } from 'react';
import { ampli, getValidatorMoveEvent } from '../../utils';
import { AddressLink, ImageIcon, StakeColumn } from '../../../components';

type TableDataGeneratorType = {
    data: Record<string, ComponentProps<typeof TableCell>>[];
    columns: {
        header: string;
        accessorKey: string;
    }[];
};

interface GenerateValidatorsTableDataArgs {
    validators: IotaValidatorSummary[];
    atRiskValidators?: [string, string][];
    validatorEvents?: IotaEvent[];
    rollingAverageApys?: ApyByValidator | null;
    limit?: number;
    showValidatorIcon?: boolean;
    columns?: TableDataGeneratorType['columns'];
}

const DEFAULT_COLUMNS: TableDataGeneratorType['columns'] = [
    { header: '#', accessorKey: 'number' },
    { header: 'Name', accessorKey: 'name' },
    { header: 'Stake', accessorKey: 'stake' },
    { header: 'Next Epoch Gas Price', accessorKey: 'nextEpochGasPrice' },
    { header: 'APY', accessorKey: 'apy' },
    { header: 'Commission', accessorKey: 'commission' },
    { header: 'Last Reward', accessorKey: 'lastReward' },
    { header: 'Voting Power', accessorKey: 'votingPower' },
    { header: 'Status', accessorKey: 'atRisk' },
];

export function generateValidatorsTableData({
    validators,
    limit,
    atRiskValidators = [],
    validatorEvents = [],
    rollingAverageApys = null,
    showValidatorIcon = true,
    columns = DEFAULT_COLUMNS,
}: GenerateValidatorsTableDataArgs): TableDataGeneratorType | null {
    const validatorsArray = limit ? validators.slice(0, limit) : validators;
    return {
        data: validatorsArray
            .sort(() => 0.5 - Math.random())
            .map((validator, i) => {
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
                    number: {
                        type: TableCellType.Text,
                        label: `${i + 1}`,
                        textColor: TableCellTextColor.Dark,
                    },
                    name: showValidatorIcon
                        ? {
                              type: TableCellType.AvatarText,
                              label: validatorName,
                              textColor: TableCellTextColor.Dark,
                              leadingElement: (
                                  <div>
                                      <ImageIcon
                                          src={img}
                                          size="sm"
                                          label={validatorName}
                                          fallback={validatorName}
                                          circle
                                      />
                                  </div>
                              ),
                          }
                        : {
                              type: TableCellType.Text,
                              label: validatorName,
                              textColor: TableCellTextColor.Dark,
                          },
                    stake: {
                        type: TableCellType.Custom,
                        renderCell: (): React.JSX.Element => <StakeColumn stake={totalStake} />,
                    },
                    nextEpochGasPrice: {
                        type: TableCellType.Custom,
                        renderCell: (): React.JSX.Element => (
                            <StakeColumn stake={validator.nextEpochGasPrice} inNano />
                        ),
                    },
                    apy: {
                        type: TableCellType.Text,
                        label: formatPercentageDisplay(apy, '--', isApyApproxZero),
                    },
                    commission: {
                        type: TableCellType.Text,
                        label: formatPercentageDisplay(Number(validator.commissionRate) / 100),
                    },
                    lastReward:
                        lastReward !== null
                            ? {
                                  type: TableCellType.Custom,
                                  renderCell: () => <StakeColumn stake={Number(lastReward)} />,
                              }
                            : {
                                  type: TableCellType.Text,
                                  label: '-',
                              },
                    votingPower: {
                        type: TableCellType.Text,
                        label: formatPercentageDisplay(Number(validator.votingPower) / 100),
                    },
                    atRisk: {
                        type: TableCellType.Badge,
                        label: isAtRisk ? 'At Risk' : 'Active',
                        badgeType: isAtRisk ? BadgeType.PrimarySolid : BadgeType.Neutral,
                    },
                    address: {
                        type: TableCellType.Custom,
                        renderCell: () => (
                            <div className="whitespace-nowrap">
                                <AddressLink
                                    address={validator.iotaAddress}
                                    noTruncate={!limit}
                                    onClick={() =>
                                        ampli.clickedValidatorRow({
                                            sourceFlow: 'Top validators - validator address',
                                            validatorAddress: validator.iotaAddress,
                                            validatorName: validatorName,
                                        })
                                    }
                                />
                            </div>
                        ),
                    },
                };
            }),
        columns,
    };
}
