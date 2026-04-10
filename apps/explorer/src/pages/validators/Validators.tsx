// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type JSX, useMemo, useState, useCallback } from 'react';
import {
    roundFloat,
    useFormatCoin,
    formatPercentageDisplay,
    useGetDynamicFields,
    useGetValidatorsApy,
    useGetValidatorsEvents,
    useMultiGetObjects,
    useMaxCommitteeSize,
} from '@iota/core';
import {
    DisplayStats,
    DisplayStatsSize,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    Title,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { ErrorBoundary, PageLayout, PlaceholderTable, TableCard } from '~/components';
import { generateValidatorsTableColumns } from '~/lib/ui';
import { Warning } from '@iota/apps-ui-icons';
import { useQuery } from '@tanstack/react-query';
import { useEnhancedRpcClient } from '~/hooks';
import { sanitizePendingValidators } from '~/lib';
import { IOTA_TYPE_ARG, normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { ValidatorFilters, ValidatorSearch, ValidatorStatusLegend } from '~/components/validator';
import type { ValidatorStatus } from '~/components/validator';
import type { IotaValidatorSummaryExtended } from '~/lib/types/validator.types';
import { useEpochProgress } from '../epochs/utils';

function ValidatorPageResult(): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');
    const {
        data: maxCommitteeSize,
        isPending: isMaxCommitteeSizePending,
        isSuccess: isMaxCommitteeSizeSuccess,
        isError: isMaxCommitteeSizeError,
    } = useMaxCommitteeSize();
    const activeValidators = data?.activeValidators;
    const numberOfValidators = activeValidators?.length || 0;
    const { label } = useEpochProgress();
    const { data: binaryVersion } = useIotaClientQuery('getRpcApiVersion');
    const [currentValidatorStatus, setCurrentValidatorStatus] = useState<ValidatorStatus>('All');
    const [searchTerm, setSearchTerm] = useState('');

    const handleStatusChange = useCallback((status: ValidatorStatus) => {
        setCurrentValidatorStatus(status);
    }, []);

    const onSearchTermChange = useCallback((term: string) => {
        setSearchTerm(term);
    }, []);

    const {
        data: validatorEvents,
        isPending: validatorsEventsLoading,
        isError: validatorEventError,
    } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });

    const { data: pendingActiveValidatorsId } = useGetDynamicFields(
        data?.pendingActiveValidatorsId || '',
    );

    const pendingValidatorsObjectIdsData = pendingActiveValidatorsId?.pages[0]?.data || [];
    const pendingValidatorsObjectIds = pendingValidatorsObjectIdsData.map((item) => item.objectId);
    const normalizedIds = pendingValidatorsObjectIds.map((id) => normalizeIotaAddress(id));

    const { data: pendingValidatorsData } = useMultiGetObjects(normalizedIds, {
        showDisplay: true,
        showContent: true,
    });

    const sanitizedPendingValidatorsData = sanitizePendingValidators(pendingValidatorsData);

    const { data: validatorsApy } = useGetValidatorsApy();
    const { data: totalSupplyData } = useIotaClientQuery('getTotalSupply', {
        coinType: IOTA_TYPE_ARG,
    });
    const { data: participationMetrics } = useIotaClientQuery('getParticipationMetrics');

    const totalStaked = useMemo(() => {
        if (!data) return 0;
        const validators = data.committeeMembers;

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

    const enhancedRpc = useEnhancedRpcClient();
    const { data: epochData } = useQuery({
        queryKey: ['epoch', data?.epoch],
        queryFn: async () => {
            const epoch = Number(data?.epoch || 0);
            // When the epoch is 0 or 1 we show the epoch 0 as the previous epoch
            // Otherwise simply use the previous epoch,
            // -1 because the cursor starts at `undefined`, and -1 to go the previous, so -1 -1 = -2
            // This is the mapping between epochs and their cursor:
            // epoch 0 = cursor undefined
            // epoch 1 = cursor 0
            // epoch 2 = cursor 1
            // ...
            return enhancedRpc.getEpochs({
                cursor: epoch === 0 || epoch === 1 ? undefined : (epoch - 2).toString(),
                limit: 1,
            });
        },
    });
    const lastEpochRewardOnAllValidators =
        epochData?.data[0].endOfEpochInfo?.totalStakeRewardsDistributed;

    const stakingRatio = (() => {
        let ratio = null;
        if (totalSupplyData?.value && totalStaked) {
            const totalSupplyValue = Number(totalSupplyData.value);
            ratio = Number(((totalStaked / totalSupplyValue) * 100).toFixed(2));
        }
        return formatPercentageDisplay(ratio);
    })();

    const activeAndPendingValidators = useMemo(() => {
        if (!data) return [];
        return Number(data.pendingActiveValidatorsSize) > 0
            ? (activeValidators?.concat(sanitizedPendingValidatorsData) ?? [])
            : (activeValidators ?? []);
    }, [data, activeValidators, sanitizedPendingValidatorsData]);

    const atRiskAddresses = useMemo(
        () => new Set(data?.atRiskValidators?.map(([address]) => address) ?? []),
        [data],
    );
    const validatorCounts = useMemo(() => {
        let committee = 0;
        let active = 0;
        let pending = 0;
        let atRisk = 0;
        for (const validator of activeAndPendingValidators as IotaValidatorSummaryExtended[]) {
            const isValidatorAtRisk = atRiskAddresses.has(validator.iotaAddress);
            const isCommitteeMember = data?.committeeMembers.some(
                (committeeMember) => committeeMember.iotaAddress === validator.iotaAddress,
            );
            if (validator.isPending) pending++;
            else if (isCommitteeMember) committee++;
            else active++;
            if (isValidatorAtRisk) atRisk++;
        }
        return { all: activeAndPendingValidators.length, active, pending, atRisk, committee };
    }, [activeAndPendingValidators, atRiskAddresses, data?.committeeMembers]);

    const filteredValidators = useMemo(
        () =>
            activeAndPendingValidators.filter((validator: IotaValidatorSummaryExtended) => {
                if (currentValidatorStatus !== 'All') {
                    const isAtRisk = atRiskAddresses.has(validator.iotaAddress);
                    const isCommitteeMember = data?.committeeMembers.some(
                        (committeeMember) => committeeMember.iotaAddress === validator.iotaAddress,
                    );
                    if (
                        currentValidatorStatus === 'Active' &&
                        (validator.isPending || isCommitteeMember)
                    )
                        return false;
                    if (currentValidatorStatus === 'Pending' && !validator.isPending) return false;
                    if (currentValidatorStatus === 'At Risk' && !isAtRisk) return false;
                    if (currentValidatorStatus === 'Committee' && !isCommitteeMember) return false;
                }
                if (searchTerm) {
                    const lower = searchTerm.toLowerCase();
                    return (
                        validator.name.toLowerCase().includes(lower) ||
                        validator.iotaAddress.toLowerCase().includes(lower)
                    );
                }
                return true;
            }),
        [activeAndPendingValidators, atRiskAddresses, currentValidatorStatus, searchTerm],
    );

    const tableColumns = useMemo(() => {
        if (!data || !maxCommitteeSize || !validatorEvents) return null;
        const includeColumns = [
            'Validator',
            'Stake',
            'APY',
            'Effective Commission',
            'Next Epoch Commission',
            'Next Epoch Stake',
            'Last Epoch Rewards',
            'Voting Power',
        ];

        return generateValidatorsTableColumns({
            allValidators: filteredValidators,
            committeeMembers: data.committeeMembers.map((validator) => validator.iotaAddress),
            atRiskValidators: data.atRiskValidators,
            maxCommitteeSize,
            validatorEvents,
            rollingAverageApys: validatorsApy,
            highlightValidatorName: true,
            includeColumns,
            currentEpoch: data.epoch,
        });
    }, [data, filteredValidators, validatorEvents, validatorsApy, maxCommitteeSize]);

    const activeCommitteeSize = data?.committeeMembers.length ?? null;
    const protocolVersion = data?.protocolVersion ?? null;

    const [formattedTotalStakedAmount, totalStakedSymbol] = useFormatCoin({
        balance: totalStaked,
    });
    const [formattedlastEpochRewardOnAllValidatorsAmount, lastEpochRewardOnAllValidatorsSymbol] =
        useFormatCoin({ balance: lastEpochRewardOnAllValidators });

    const validatorsMainStats = [
        {
            title: 'Committee Stake',
            value: formattedTotalStakedAmount,
            supportingLabel: totalStakedSymbol,
            tooltipText:
                "The combined amount of tokens staked with validators selected for the upcoming epoch's active committee.",
        },
        {
            title: 'Staking Ratio',
            value: stakingRatio,
            tooltipText:
                "The proportion of the total IOTA supply delegated to the validators chosen for the next epoch's active committee.",
        },
    ];

    const validatorsSecondaryStats = [
        {
            title: 'AVG APY',
            value: averageAPY ? `${averageAPY}%` : '--',
            tooltipText:
                'The overall average Annual Percentage Yield (APY) across all participating validators.',
        },
        {
            title: 'Delegators',
            value: participationMetrics ? participationMetrics?.totalAddresses : undefined,
            supportingLabel: participationMetrics ? undefined : 'Coming Soon',
            tooltipText:
                'Total number of unique addresses that have delegated stake in the current epoch.',
        },
        {
            title: 'Last Epoch Rewards',
            value: lastEpochRewardOnAllValidators
                ? formattedlastEpochRewardOnAllValidatorsAmount
                : '--',
            supportingLabel: formattedlastEpochRewardOnAllValidatorsAmount
                ? lastEpochRewardOnAllValidatorsSymbol
                : undefined,
            tooltipText: 'The staking rewards earned in the previous epoch.',
        },
        {
            title: 'Current Epoch',
            value: data?.epoch ?? '--',
            supportingLabel: label ?? '--',
        },
        {
            title: 'Protocol Version',
            value: protocolVersion ?? '--',
            supportingLabel: binaryVersion ? `v${binaryVersion}` : '--',
        },
        {
            title: 'Active Validators',
            value: numberOfValidators || '--',
        },
        {
            title: 'Active Committee Size',
            value: activeCommitteeSize ?? '--',
        },
        {
            title: 'Max Committee Size',
            value: maxCommitteeSize ?? '--',
        },
    ];

    return (
        <PageLayout
            content={
                isError || isMaxCommitteeSizeError || validatorEventError ? (
                    <InfoBox
                        title="Failed to load data"
                        supportingText="Validator data could not be loaded"
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : (
                    <div className="flex w-full flex-col gap-xl">
                        <div className="pt-md--rs text-display-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                            Validators
                        </div>

                        <div className="grid grid-cols-1 gap-md--rs md:grid-cols-2">
                            {validatorsMainStats.map((stat) => (
                                <DisplayStats
                                    key={stat.title}
                                    label={stat.title}
                                    tooltipText={stat.tooltipText}
                                    value={stat.value}
                                    supportingLabel={stat.supportingLabel}
                                    size={DisplayStatsSize.Large}
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            ))}
                        </div>

                        <div className="grid grid-cols-1 gap-md--rs sm:grid-cols-2 md:grid-cols-4">
                            {validatorsSecondaryStats.map((stat) => (
                                <DisplayStats
                                    key={stat.title}
                                    label={stat.title}
                                    tooltipText={stat.tooltipText}
                                    value={stat.value}
                                    supportingLabel={stat.supportingLabel}
                                    size={DisplayStatsSize.Default}
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            ))}
                        </div>
                        <Panel>
                            <Title title="All Validators" />

                            <div className="flex flex-col gap-md p-md">
                                <ValidatorSearch onSearch={onSearchTermChange} />
                                <div className="flex">
                                    <ValidatorFilters
                                        selectedStatus={currentValidatorStatus}
                                        onStatusChange={handleStatusChange}
                                        validatorCounts={validatorCounts}
                                    />
                                </div>
                            </div>
                            <div className="p-md">
                                <ErrorBoundary>
                                    {(isPending ||
                                        isMaxCommitteeSizePending ||
                                        validatorsEventsLoading) && (
                                        <PlaceholderTable
                                            rowCount={20}
                                            rowHeight="13px"
                                            colHeadings={[
                                                'Validator',
                                                'Stake',
                                                'APY',
                                                'Effective Commission',
                                                'Last Epoch Rewards',
                                                'Voting Power',
                                                'Status',
                                            ]}
                                        />
                                    )}
                                    {isSuccess &&
                                        isMaxCommitteeSizeSuccess &&
                                        filteredValidators &&
                                        tableColumns && (
                                            <TableCard
                                                sortTable
                                                defaultSorting={[
                                                    { id: 'stakingPoolIotaBalance', desc: true },
                                                ]}
                                                data={filteredValidators}
                                                columns={tableColumns}
                                                areHeadersCentered={false}
                                            />
                                        )}
                                </ErrorBoundary>
                            </div>
                        </Panel>
                        <ValidatorStatusLegend />
                    </div>
                )
            }
        />
    );
}

export { ValidatorPageResult };
