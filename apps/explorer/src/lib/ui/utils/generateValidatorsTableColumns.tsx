// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeSize, BadgeType, TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import type { ColumnDef, Row } from '@tanstack/react-table';
import {
    type ApyByValidator,
    formatPercentageDisplay,
    getValidatorEffectiveCommission,
    ImageIcon,
    ImageIconSize,
    useCopyToClipboard,
} from '@iota/core';
import { ampli, getValidatorMoveEvent, type IotaValidatorSummaryExtended } from '~/lib';
import { StakeColumn } from '~/components';
import type { IotaEvent, IotaValidatorSummary } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { ValidatorLink } from '~/components/ui';
import { Copy } from '@iota/apps-ui-icons';

interface GenerateValidatorsTableColumnsArgs {
    allValidators?: IotaValidatorSummary[];
    committeeMembers?: string[];
    atRiskValidators?: [string, string][];
    maxCommitteeSize?: number;
    validatorEvents?: IotaEvent[];
    rollingAverageApys?: ApyByValidator;
    showValidatorIcon?: boolean;
    includeColumns?: string[];
    highlightValidatorName?: boolean;
    currentEpoch?: string;
}

function ValidatorWithImage({
    validator,
    highlightValidatorName,
    committeeMembers = [],
    atRiskAddresses = new Set(),
}: {
    validator: IotaValidatorSummaryExtended;
    highlightValidatorName?: boolean;
    committeeMembers?: string[];
    atRiskAddresses?: Set<string>;
}) {
    const validatorAddress = validator.iotaAddress;
    const isValidatorCommitteeMember = committeeMembers.includes(validatorAddress);
    const isAtRisk = atRiskAddresses.has(validatorAddress);
    const truncatedAddress = `${validatorAddress.slice(0, 8)}\u2026${validatorAddress.slice(-6)}`;
    const copyToClipboard = useCopyToClipboard();

    const statusBadges = validator.isPending
        ? [{ type: BadgeType.Warning, label: 'Pending' }]
        : isValidatorCommitteeMember
          ? [{ type: BadgeType.Success, label: 'Committee' }]
          : [{ type: BadgeType.PrimarySoft, label: 'Active' }];

    if (isAtRisk) {
        statusBadges.push({ type: BadgeType.Error, label: 'At Risk' });
    }

    const validatorNameContainer = (
        <div className="flex min-w-0 flex-col gap-0.5">
            <div className="flex flex-wrap items-center gap-1.5">
                <span
                    className={clsx('truncate text-label-lg', {
                        'text-iota-neutral-10 dark:text-iota-neutral-92': highlightValidatorName,
                        'text-iota-neutral-40 dark:text-iota-neutral-60': !highlightValidatorName,
                    })}
                >
                    {validator.name}
                </span>
                {statusBadges.map((badge) => (
                    <Badge
                        key={badge.label}
                        type={badge.type}
                        label={badge.label}
                        size={BadgeSize.Small}
                    />
                ))}
            </div>
            <div className="flex items-center gap-1">
                <span className="text-label-sm tabular-nums text-iota-neutral-40 dark:text-iota-neutral-60">
                    {truncatedAddress}
                </span>
                <button
                    type="button"
                    aria-label="Copy address"
                    className="flex items-center text-iota-neutral-40 transition-colors hover:text-iota-neutral-10 dark:text-iota-neutral-60 dark:hover:text-iota-neutral-92"
                    onClick={(e) => {
                        e.stopPropagation();
                        e.preventDefault();
                        copyToClipboard(validatorAddress);
                    }}
                >
                    <Copy className="h-3 w-3" />
                </button>
            </div>
        </div>
    );

    const avatarElement = (
        <div className="h-8 w-8 shrink-0">
            <ImageIcon
                src={validator.imageUrl}
                label={validator.name}
                fallback={validator.name}
                size={ImageIconSize.Medium}
                rounded
            />
        </div>
    );

    return validator.isPending ? (
        <div className="flex items-center gap-x-2.5 text-iota-neutral-40 dark:text-iota-neutral-60">
            {avatarElement}
            {validatorNameContainer}
        </div>
    ) : (
        <ValidatorLink
            address={validator.iotaAddress}
            showAddressAlias={false}
            onClick={() =>
                ampli.clickedValidatorRow({
                    sourceFlow: 'Epoch details',
                    validatorAddress: validator.iotaAddress,
                    validatorName: validator.name,
                })
            }
            label={
                <div className="flex items-center gap-x-2.5 text-iota-neutral-40 dark:text-iota-neutral-60">
                    {avatarElement}
                    {validatorNameContainer}
                </div>
            }
        />
    );
}

export function generateValidatorsTableColumns({
    committeeMembers = [],
    atRiskValidators = [],
    validatorEvents = [],
    rollingAverageApys,
    showValidatorIcon = true,
    includeColumns,
    highlightValidatorName,
    currentEpoch,
}: GenerateValidatorsTableColumnsArgs): ColumnDef<IotaValidatorSummaryExtended>[] {
    const atRiskAddressSet = new Set(atRiskValidators.map(([address]) => address));

    let columns: ColumnDef<IotaValidatorSummaryExtended>[] = [
        {
            header: 'Validator',
            id: 'name',
            accessorKey: 'name',
            enableSorting: true,
            sortingFn: (row1, row2, columnId) => {
                const value1 = row1.getValue<string>(columnId);
                const value2 = row2.getValue<string>(columnId);
                return sortByString(value1, value2);
            },
            cell({ row }) {
                const { original: validator } = row;
                return (
                    <TableCellBase>
                        {showValidatorIcon ? (
                            <ValidatorWithImage
                                validator={validator}
                                highlightValidatorName={highlightValidatorName}
                                committeeMembers={committeeMembers}
                                atRiskAddresses={atRiskAddressSet}
                            />
                        ) : (
                            <TableCellText>
                                <span
                                    className={
                                        highlightValidatorName
                                            ? 'text-iota-neutral-10 dark:text-iota-neutral-92'
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
            cell({ getValue, row }) {
                const validator = row.original as IotaValidatorSummaryExtended;
                if (validator.isCandidate || validator.isPending) {
                    return (
                        <TableCellBase>
                            <TableCellText>--</TableCellText>
                        </TableCellBase>
                    );
                }
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
            header: 'Effective Commission',
            accessorKey: 'effectiveCommissionRate',
            id: 'effectiveCommissionRate',
            enableSorting: true,
            sortingFn: sortByNumber,
            cell({ row }) {
                return (
                    <TableCellBase>
                        <TableCellText>
                            {getValidatorEffectiveCommission(row.original)}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },

        {
            header: 'Voting Power',
            meta: {
                tooltip:
                    "This validator's share of total committee voting power, proportional to its stake. Determines influence over consensus.",
            },
            accessorKey: 'votingPower',
            enableSorting: true,
            sortingFn: sortByNumber,
            cell({ getValue, row }) {
                const validator = row.original as IotaValidatorSummaryExtended;
                const votingPower = getValue<string>();
                const commission = Number(votingPower);
                return (
                    <TableCellBase>
                        <TableCellText>
                            {validator.isCandidate || validator.isPending || isNaN(commission)
                                ? '--'
                                : `${commission / 100}%`}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Next Epoch Stake',
            accessorKey: 'nextEpochStake',
            id: 'nextEpochStake',
            enableSorting: true,
            sortingFn: (rowA, rowB, columnId) =>
                BigInt(rowA.getValue(columnId)) - BigInt(rowB.getValue(columnId)) > 0 ? 1 : -1,
            cell({ getValue }) {
                const nextEpochStake = getValue<string>();
                const isValid = nextEpochStake && !isNaN(Number(nextEpochStake));
                return (
                    <TableCellBase>
                        {isValid ? (
                            <StakeColumn stake={nextEpochStake} />
                        ) : (
                            <TableCellText>--</TableCellText>
                        )}
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Last Epoch Rewards',
            meta: {
                tooltip:
                    "Total staking rewards distributed to this validator's pool at the end of the previous epoch.",
            },
            accessorKey: 'lastReward',
            id: 'lastReward',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const lastRewardA = getLastReward(validatorEvents, rowA, currentEpoch);
                const lastRewardB = getLastReward(validatorEvents, rowB, currentEpoch);

                if (lastRewardA === null && lastRewardB === null) return 0;
                if (lastRewardA === null) return 1;
                if (lastRewardB === null) return -1;

                return lastRewardA > lastRewardB ? -1 : 1;
            },
            cell({ row }) {
                const lastReward = getLastReward(validatorEvents, row, currentEpoch);
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
            cell({ getValue, row }) {
                const validator = row.original as IotaValidatorSummaryExtended;
                const votingPower = getValue<string>();
                const numericPower = Number(votingPower);
                return (
                    <TableCellBase>
                        <TableCellText>
                            {validator.isCandidate ||
                            validator.isPending ||
                            !votingPower ||
                            isNaN(numericPower)
                                ? '--'
                                : numericPower / 100 + '%'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Status',
            accessorKey: 'status',
            id: 'status',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const { label: labelA } = determineRisk(committeeMembers, atRiskValidators, rowA);
                const { label: labelB } = determineRisk(committeeMembers, atRiskValidators, rowB);
                return sortByString(labelA, labelB);
            },
            cell({ row }) {
                const { atRisk, label, isPending, isCandidate } = determineRisk(
                    committeeMembers,
                    atRiskValidators,
                    row,
                );

                if (isPending || isCandidate) {
                    return (
                        <TableCellBase>
                            <Badge type={BadgeType.Neutral} label={label} />
                        </TableCellBase>
                    );
                }

                return (
                    <TableCellBase>
                        <Badge
                            type={
                                atRisk === null
                                    ? BadgeType.Success
                                    : atRisk > 1
                                      ? BadgeType.Warning
                                      : BadgeType.Error
                            }
                            label={label}
                        />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Current Epoch Rewards',
            accessorKey: 'isEarningCurrent',
            id: 'isEarningCurrent',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const isCommitteeMemberA = committeeMembers.some(
                    (address) => address === rowA.original.iotaAddress,
                );
                const isCommitteeMemberB = committeeMembers.some(
                    (address) => address === rowB.original.iotaAddress,
                );
                return sortByBoolean(isCommitteeMemberA, isCommitteeMemberB);
            },
            cell({ row }) {
                const isCommitteeMember = committeeMembers.find(
                    (committeeMemberAddress) => committeeMemberAddress === row.original.iotaAddress,
                );
                const label = isCommitteeMember ? 'Earning' : 'Not Earning';
                return (
                    <TableCellBase>
                        <Badge
                            type={isCommitteeMember ? BadgeType.PrimarySoft : BadgeType.Neutral}
                            label={label}
                        />
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Next Epoch Rewards',
            accessorKey: 'isEarningNext',
            id: 'isEarningNext',
            enableSorting: true,
            sortingFn: (rowA, rowB) => {
                const valA = rowA.original as IotaValidatorSummaryExtended;
                const valB = rowB.original as IotaValidatorSummaryExtended;

                // Candidates and pending validators never earn
                if (valA.isCandidate || valA.isPending) {
                    return valB.isCandidate || valB.isPending ? 0 : -1;
                }
                if (valB.isCandidate || valB.isPending) {
                    return 1;
                }

                const { atRisk: atRiskA } = determineRisk(committeeMembers, atRiskValidators, rowA);
                const { atRisk: atRiskB } = determineRisk(committeeMembers, atRiskValidators, rowB);

                const isInTopStakersA = topValidators.some(
                    (v) => v.iotaAddress === rowA.original.iotaAddress,
                );
                const isInTopStakersB = topValidators.some(
                    (v) => v.iotaAddress === rowB.original.iotaAddress,
                );

                const isEarningNextA = (atRiskA === null || atRiskA > 1) && isInTopStakersA;
                const isEarningNextB = (atRiskB === null || atRiskB > 1) && isInTopStakersB;

                return sortByBoolean(isEarningNextA, isEarningNextB);
            },
            cell({ row }) {
                const validator = row.original as IotaValidatorSummaryExtended;

                // Candidates and pending validators are not part of the active set
                // and cannot earn rewards in the next epoch.
                if (validator.isCandidate || validator.isPending) {
                    return (
                        <TableCellBase>
                            <Badge type={BadgeType.Neutral} label="Not Earning" />
                        </TableCellBase>
                    );
                }

                const { atRisk } = determineRisk(committeeMembers, atRiskValidators, row);

                const isInTopStakers = !!topValidators.find(
                    (v) => v.iotaAddress === row.original.iotaAddress,
                );

                // if its active validator, not at high risk,
                // and is part of the top X stakers,
                // it will generate rewards in the next epoch, otherwise not.
                const isEarningNext = (atRisk === null || atRisk > 1) && isInTopStakers;

                const label = isEarningNext ? 'Earning' : 'Not Earning';

                return (
                    <TableCellBase>
                        <Badge
                            type={isEarningNext ? BadgeType.PrimarySoft : BadgeType.Neutral}
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
    currentEpoch?: string,
): number | null {
    const { original: validator } = row;
    const event = getValidatorMoveEvent(validatorEvents, validator.iotaAddress, currentEpoch) as {
        pool_staking_reward?: string;
    };
    return event?.pool_staking_reward ? Number(event.pool_staking_reward) : null;
}
