// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    BadgeType,
    type TableCellProps,
    TableCellType,
    TableCellTextColor,
    CellAvatarText,
} from '@iota/apps-ui-kit';
import type { ColumnDef } from '@tanstack/react-table';
import { type ApyByValidator, formatPercentageDisplay } from '@iota/core';

import { ampli, getValidatorMoveEvent, VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib';
import { Link, StakeColumn, AddressLink, ImageIcon } from '~/components';
import type { IotaEvent, IotaValidatorSummary } from '@iota/iota-sdk/dist/cjs/client';

interface ValidatorTableRow {
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
    accentValidatorName?: boolean;
}

const ALL_VALIDATOR_COLUMNS = [
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
        header: 'Address',
        accessorKey: 'address',
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
] as const;

type AccessorKey = (typeof ALL_VALIDATOR_COLUMNS)[number]['accessorKey'];

export interface ValidatorTableColumn {
    header: string;
    accessorKey: AccessorKey;
}

const DEFAULT_COLUMNS: ValidatorTableColumn[] = [
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

function generateValidatorAtRisk(atRisk: number | null): TableCellProps {
    if (atRisk === null) {
        return {
            type: TableCellType.Badge,
            badgeType: BadgeType.PrimarySoft,
            label: 'Active',
        };
    }

    const atRiskText = atRisk > 1 ? `in ${atRisk} epochs` : 'next epoch';
    return {
        type: TableCellType.Badge,
        badgeType: BadgeType.Neutral,
        label: `At Risk ${atRiskText}`,
    };
}

interface ValidatorNameProps {
    address: string;
    name: string;
    imageUrl: string;
    accentValidatorName?: boolean;
}
function ValidatorName({ address, name, imageUrl, accentValidatorName }: ValidatorNameProps) {
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
            <CellAvatarText
                leadingElement={<ImageIcon src={imageUrl} size="sm" label={name} fallback={name} />}
                label={name}
                textColor={
                    accentValidatorName ? TableCellTextColor.Dark : TableCellTextColor.Default
                }
            />
        </Link>
    );
}

function ValidatorAddress({
    address,
    name,
    limit,
}: {
    address: string;
    name: string;
    limit?: number;
}) {
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
    accentValidatorName,
}: GenerateValidatorsTableDataArgs): {
    data: ValidatorTableRow[];
    columns: ColumnDef<object, unknown>[];
} {
    return {
        data: validators.map((validator, i) => {
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
                          children: (
                              <ValidatorName
                                  address={validator.iotaAddress}
                                  name={validatorName}
                                  imageUrl={validator.imageUrl}
                              />
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
                    label: validator.votingPower ? Number(validator.votingPower) / 100 + '%' : '--',
                },
                atRisk: generateValidatorAtRisk(
                    isAtRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRiskValidator[1]) : null,
                ),
                address: {
                    type: TableCellType.Children,
                    children: (
                        <ValidatorAddress
                            address={validator.iotaAddress}
                            name={validatorName}
                            limit={limit}
                        />
                    ),
                },
            };
        }),
        columns: columns,
    };
}
