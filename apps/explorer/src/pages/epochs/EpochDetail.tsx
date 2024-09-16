// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, useFormatCoin } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { LoadingIndicator } from '@iota/ui';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';
import {
    ButtonSegment,
    ButtonSegmentType,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';

import { CheckpointsTable, PageLayout } from '~/components';
import { Banner, Stats, type StatsProps, TableCard } from '~/components/ui';
import { useEnhancedRpcClient } from '~/hooks/useEnhancedRpc';
import { getEpochStorageFundFlow, getSupplyChangeAfterEpochEnd } from '~/lib/utils';
import { EpochProgress } from './stats/EpochProgress';
import { EpochStats } from './stats/EpochStats';
import { ValidatorStatus } from './stats/ValidatorStatus';
import {
    generateValidatorsTableData,
    type ValidatorTableColumn,
} from '~/lib/ui/utils/generateValidatorsTableData';

export const VALIDATOR_COLUMNS: ValidatorTableColumn[] = [
    {
        header: 'Name',
        accessorKey: 'name',
    },
    {
        header: 'Stake',
        accessorKey: 'stake',
    },
    {
        header: 'Proposed next Epoch gas price',
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
        header: 'Last Epoch Reward',
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

function IotaStats({
    amount,
    showSign,
    ...props
}: Omit<StatsProps, 'children'> & {
    amount: bigint | number | string | undefined | null;
    showSign?: boolean;
}): JSX.Element {
    const [formattedAmount, symbol] = useFormatCoin(
        amount,
        IOTA_TYPE_ARG,
        CoinFormat.ROUNDED,
        showSign,
    );

    return (
        <Stats postfix={formattedAmount && symbol} {...props}>
            {formattedAmount || '--'}
        </Stats>
    );
}

enum EpochTabs {
    Checkpoints = 'checkpoints',
    Validators = 'validators',
}

export default function EpochDetail() {
    const [activeTabId, setActiveTabId] = useState(EpochTabs.Checkpoints);
    const { id } = useParams();
    const enhancedRpc = useEnhancedRpcClient();
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const { data, isPending, isError } = useQuery({
        queryKey: ['epoch', id],
        queryFn: async () =>
            enhancedRpc.getEpochs({
                // todo: endpoint returns no data for epoch 0
                cursor: id === '0' ? undefined : (Number(id!) - 1).toString(),
                limit: 1,
            }),
    });

    const [epochData] = data?.data ?? [];
    const isCurrentEpoch = useMemo(
        () => systemState?.epoch === epochData?.epoch,
        [systemState, epochData],
    );

    const validatorsTable = useMemo(() => {
        if (!epochData?.validators || epochData.validators.length === 0) return null;
        // todo: enrich this historical validator data when we have
        // at-risk / pending validators for historical epochs
        return generateValidatorsTableData({
            validators: [...epochData.validators].sort(() => 0.5 - Math.random()),
            atRiskValidators: [],
            validatorEvents: [],
            rollingAverageApys: null,
            columns: VALIDATOR_COLUMNS,
            showValidatorIcon: true,
        });
    }, [epochData]);

    if (isPending) return <PageLayout content={<LoadingIndicator />} />;

    if (isError || !epochData)
        return (
            <PageLayout
                content={
                    <Banner variant="error" fullWidth>
                        {`There was an issue retrieving data for epoch ${id}.`}
                    </Banner>
                }
            />
        );

    const { fundInflow, fundOutflow, netInflow } = getEpochStorageFundFlow(
        epochData.endOfEpochInfo,
    );

    // cursor should be the sequence number of the last checkpoint + 1  if we want to query with desc. order
    const initialCursorPlusOne = epochData.endOfEpochInfo?.lastCheckpointId
        ? (Number(epochData.endOfEpochInfo?.lastCheckpointId) + 1).toString()
        : undefined;

    return (
        <PageLayout
            content={
                <div className="flex flex-col space-y-16">
                    <div className="grid grid-flow-row gap-4 sm:gap-2 md:flex md:gap-6">
                        <div className="flex min-w-[136px] max-w-[240px]">
                            <EpochProgress
                                epoch={epochData.epoch}
                                inProgress={isCurrentEpoch}
                                start={Number(epochData.epochStartTimestamp)}
                                end={Number(epochData.endOfEpochInfo?.epochEndTimestamp ?? 0)}
                            />
                        </div>

                        <EpochStats label="Rewards">
                            <IotaStats
                                label="Total Stake"
                                tooltip=""
                                amount={epochData.endOfEpochInfo?.totalStake}
                            />
                            <IotaStats
                                label="Stake Rewards"
                                amount={epochData.endOfEpochInfo?.totalStakeRewardsDistributed}
                            />
                            <IotaStats
                                label="Gas Fees"
                                amount={epochData.endOfEpochInfo?.totalGasFees}
                            />
                        </EpochStats>

                        <EpochStats label="Storage Fund Balance">
                            <IotaStats
                                label="Fund Size"
                                amount={epochData.endOfEpochInfo?.storageFundBalance}
                            />
                            <IotaStats label="Net Inflow" amount={netInflow} />
                            <IotaStats label="Fund Inflow" amount={fundInflow} />
                            <IotaStats label="Fund Outflow" amount={fundOutflow} />
                        </EpochStats>

                        <EpochStats label="Supply">
                            <IotaStats
                                label="Supply Change"
                                amount={getSupplyChangeAfterEpochEnd(epochData.endOfEpochInfo)}
                                showSign
                            />
                        </EpochStats>

                        {isCurrentEpoch ? <ValidatorStatus /> : null}
                    </div>

                    <div className="rounded-xl bg-white">
                        <div className="relative">
                            <div className="border-shader-inverte-dark-8 absolute bottom-0 left-0 z-0 h-[1px] w-full border-b" />
                            <SegmentedButton
                                type={SegmentedButtonType.Transparent}
                                shape={ButtonSegmentType.Underlined}
                            >
                                <ButtonSegment
                                    type={ButtonSegmentType.Underlined}
                                    label="Checkpoints"
                                    selected={activeTabId === EpochTabs.Checkpoints}
                                    onClick={() => setActiveTabId(EpochTabs.Checkpoints)}
                                />
                                <ButtonSegment
                                    type={ButtonSegmentType.Underlined}
                                    label="Participating Validators"
                                    selected={activeTabId === EpochTabs.Validators}
                                    onClick={() => setActiveTabId(EpochTabs.Validators)}
                                />
                            </SegmentedButton>
                        </div>
                        <div className="px-lg py-md">
                            {activeTabId === EpochTabs.Checkpoints ? (
                                <CheckpointsTable
                                    initialCursor={initialCursorPlusOne}
                                    maxCursor={epochData.firstCheckpointId}
                                    initialLimit={20}
                                />
                            ) : null}
                            {activeTabId === EpochTabs.Validators && validatorsTable ? (
                                <TableCard
                                    data={validatorsTable.data}
                                    columns={validatorsTable.columns}
                                />
                            ) : null}
                        </div>
                    </div>
                </div>
            }
        />
    );
}
