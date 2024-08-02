// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { LoadingIndicator } from '@iota/ui';
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useParams } from 'react-router-dom';

import { CheckpointsTable, PageLayout } from '~/components';
import {
    Banner,
    Stats,
    TableCard,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
    type StatsProps,
} from '~/components/ui';
import { useEnhancedRpcClient } from '~/hooks/useEnhancedRpc';
import { getEpochStorageFundFlow } from '~/lib/utils';
import { validatorsTableData } from '../validators/Validators';
import { EpochProgress } from './stats/EpochProgress';
import { EpochStats } from './stats/EpochStats';
import { ValidatorStatus } from './stats/ValidatorStatus';

function IotaStats({
    amount,
    ...props
}: Omit<StatsProps, 'children'> & {
    amount: bigint | number | string | undefined | null;
}): JSX.Element {
    const [formattedAmount, symbol] = useFormatCoin(amount, IOTA_TYPE_ARG);

    return (
        <Stats postfix={formattedAmount && symbol} {...props}>
            {formattedAmount || '--'}
        </Stats>
    );
}

export default function EpochDetail() {
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
        if (!epochData?.validators) return null;
        // todo: enrich this historical validator data when we have
        // at-risk / pending validators for historical epochs
        return validatorsTableData(
            [...epochData.validators].sort(() => 0.5 - Math.random()),
            [],
            [],
            null,
        );
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

                        {isCurrentEpoch ? <ValidatorStatus /> : null}
                    </div>

                    <Tabs size="lg" defaultValue="checkpoints">
                        <TabsList>
                            <TabsTrigger value="checkpoints">Checkpoints</TabsTrigger>
                            <TabsTrigger value="validators">Participating Validators</TabsTrigger>
                        </TabsList>
                        <TabsContent value="checkpoints">
                            <CheckpointsTable
                                initialCursor={initialCursorPlusOne}
                                maxCursor={epochData.firstCheckpointId}
                                initialLimit={20}
                            />
                        </TabsContent>
                        <TabsContent value="validators">
                            {validatorsTable ? (
                                <TableCard
                                    data={validatorsTable.data}
                                    columns={validatorsTable.columns}
                                    sortTable
                                />
                            ) : null}
                        </TabsContent>
                    </Tabs>
                </div>
            }
        />
    );
}
