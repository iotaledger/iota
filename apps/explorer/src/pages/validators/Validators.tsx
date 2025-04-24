// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type JSX, useMemo } from 'react';
import {
    roundFloat,
    useFormatCoin,
    formatPercentageDisplay,
    useGetDynamicFields,
    useGetValidatorsApy,
    useGetValidatorsEvents,
    useMultiGetObjects,
} from '@iota/core';
import {
    Badge,
    BadgeType,
    DisplayStats,
    DisplayStatsSize,
    DisplayStatsType,
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

function ValidatorPageResult(): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');
    const activeValidators = data?.activeValidators;
    const numberOfValidators = activeValidators?.length || 0;

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

    const sanitizePendingValidatorsData = sanitizePendingValidators(pendingValidatorsData);

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

    const tableData = data
        ? Number(data.pendingActiveValidatorsSize) > 0
            ? activeValidators?.concat(sanitizePendingValidatorsData)
            : activeValidators
        : [];

    const tableColumns = useMemo(() => {
        if (!data || !validatorEvents) return null;
        const includeColumns = [
            'Name',
            'Stake',
            'APY',
            'Commission',
            'Last Epoch Rewards',
            'Voting Power',
            'Status',
        ];

        return generateValidatorsTableColumns({
            committeeMembers: data.committeeMembers.map((validator) => validator.iotaAddress),
            atRiskValidators: data.atRiskValidators,
            validatorEvents,
            rollingAverageApys: validatorsApy,
            highlightValidatorName: true,
            includeColumns,
            currentEpoch: data.epoch,
        });
    }, [data, validatorEvents, validatorsApy]);

    const [formattedTotalStakedAmount, totalStakedSymbol] = useFormatCoin({ balance: totalStaked });
    const [formattedlastEpochRewardOnAllValidatorsAmount, lastEpochRewardOnAllValidatorsSymbol] =
        useFormatCoin({ balance: lastEpochRewardOnAllValidators });

    const validatorStats = [
        {
            title: 'Total Staked',
            value: formattedTotalStakedAmount,
            supportingLabel: totalStakedSymbol,
            tooltipText:
                'The combined IOTA staked by validators (committee) and delegators on the network to support validation and generate rewards.',
        },
        {
            title: 'Participation',
            value: participationMetrics ? participationMetrics?.totalAddresses : undefined,
            supportingLabel: participationMetrics ? undefined : 'Coming Soon',
            tooltipText:
                'Total number of unique addresses that have delegated stake in the current epoch. Includes both staked and timelocked staked IOTA',
        },
        {
            title: 'Staking Ratio',
            value: stakingRatio,
            tooltipText: 'The ratio of the total staked IOTA to the total supply of IOTA.',
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
                    <InfoBox
                        title="Failed to load data"
                        supportingText="Validator data could not be loaded"
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : (
                    <div className="flex w-full flex-col gap-xl">
                        <div className="py-md--rs text-display-sm text-neutral-10 dark:text-neutral-92">
                            Validators
                        </div>
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
                        <Panel>
                            <Title
                                title="All Validators"
                                supportingElement={
                                    <span className="ml-1">
                                        <Badge
                                            type={BadgeType.PrimarySoft}
                                            label={numberOfValidators.toString()}
                                        />
                                    </span>
                                }
                            />
                            <div className="p-md">
                                <ErrorBoundary>
                                    {(isPending || validatorsEventsLoading) && (
                                        <PlaceholderTable
                                            rowCount={20}
                                            rowHeight="13px"
                                            colHeadings={['Name', 'Address', 'Stake']}
                                        />
                                    )}
                                    {isSuccess && tableData && tableColumns && (
                                        <TableCard
                                            sortTable
                                            defaultSorting={[
                                                { id: 'stakingPoolIotaBalance', desc: true },
                                            ]}
                                            data={tableData}
                                            columns={tableColumns}
                                            areHeadersCentered={false}
                                        />
                                    )}
                                </ErrorBoundary>
                            </div>
                        </Panel>
                    </div>
                )
            }
        />
    );
}

export { ValidatorPageResult };
