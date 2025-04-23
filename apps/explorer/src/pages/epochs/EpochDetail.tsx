// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';
import {
    ButtonSegment,
    ButtonSegmentType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';

import { CheckpointsTable, PageLayout } from '~/components';
import { TableCard } from '~/components/ui';
import { useEnhancedRpcClient } from '~/hooks/useEnhancedRpc';
import { EpochStats, EpochStatsGrid } from './stats/EpochStats';
import { ValidatorStatus } from './stats/ValidatorStatus';
import { generateValidatorsTableColumns } from '~/lib/ui/utils/generateValidatorsTableColumns';
import cx from 'clsx';
import { TokenStats } from './stats/TokenStats';
import { EpochTopStats } from './stats/EpochTopStats';
import { getEpochStorageFundFlow } from '~/lib/utils';
import { Warning } from '@iota/apps-ui-icons';
import { VALIDATORS_EVENTS_QUERY } from '@iota/core';
import { useEndOfEpochTransactionFromCheckpoint } from '~/hooks/useEndOfEpochTransactionFromCheckpoint';
import { type IotaEvent } from '@iota/iota-sdk/src/client';
import { useIotaClientQuery } from '@iota/dapp-kit';

enum EpochTabs {
    Checkpoints = 'checkpoints',
    Validators = 'validators',
}

export function EpochDetail() {
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

    const endOfPreviousEpochCheckpoint = epochData?.firstCheckpointId
        ? (Number(epochData.firstCheckpointId) - 1).toString()
        : undefined;
    const { data: endOfEpochTransaction } = useEndOfEpochTransactionFromCheckpoint(
        endOfPreviousEpochCheckpoint,
    );
    const validatorEvents: IotaEvent[] | undefined = endOfEpochTransaction?.events?.filter(
        (event): event is IotaEvent => event.type === VALIDATORS_EVENTS_QUERY,
    );

    const isCurrentEpoch = useMemo(
        () => systemState?.epoch === epochData?.epoch,
        [systemState, epochData],
    );
    const committeeMembers =
        epochData?.committeeMembers?.map(
            (committeeMemberIndex) => epochData.validators[Number(committeeMemberIndex)],
        ) ?? [];

    const tableColumns = useMemo(() => {
        if (!epochData?.validators || epochData.validators.length === 0) return null;
        const includeColumns = [
            'Name',
            'Stake',
            'APY',
            'Commission',
            'Last Epoch Rewards',
            'Voting Power',
            'Status',
        ];

        // todo: enrich this historical validator data when we have
        // at-risk / pending validators for historical epochs
        return generateValidatorsTableColumns({
            committeeMembers: committeeMembers.map((member) => member.iotaAddress),
            validatorEvents: validatorEvents ?? [],
            showValidatorIcon: true,
            includeColumns,
            currentEpoch: epochData.epoch,
        });
    }, [epochData, validatorEvents, committeeMembers]);

    if (isPending) return <PageLayout content={<LoadingIndicator />} />;

    if (isError || !epochData)
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Failed to load epoch data"
                        supportingText={`There was an issue retrieving data for epoch ${id}`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
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
                <div className="flex flex-col gap-2xl">
                    <div
                        className={cx(
                            'grid grid-cols-1 gap-md--rs',
                            isCurrentEpoch ? 'md:grid-cols-2' : 'md:grid-cols-3',
                        )}
                    >
                        <EpochStats
                            title={`Epoch ${epochData.epoch}`}
                            subtitle={isCurrentEpoch ? 'In progress' : 'Ended'}
                        >
                            <EpochTopStats
                                inProgress={isCurrentEpoch}
                                start={Number(epochData.epochStartTimestamp)}
                                end={Number(epochData.endOfEpochInfo?.epochEndTimestamp ?? 0)}
                                endOfEpochInfo={epochData.endOfEpochInfo}
                            />
                        </EpochStats>
                        {!isCurrentEpoch && (
                            <>
                                <EpochStats title="Rewards">
                                    <EpochStatsGrid>
                                        <TokenStats
                                            label="Total Stake"
                                            amount={epochData.endOfEpochInfo?.totalStake}
                                        />
                                        <TokenStats
                                            label="Stake Rewards"
                                            amount={
                                                epochData.endOfEpochInfo
                                                    ?.totalStakeRewardsDistributed
                                            }
                                        />
                                        <TokenStats
                                            label="Gas Fees"
                                            amount={epochData.endOfEpochInfo?.totalGasFees}
                                        />
                                    </EpochStatsGrid>
                                </EpochStats>

                                <EpochStats title="Storage Fund Balance">
                                    <EpochStatsGrid>
                                        <TokenStats
                                            label="Fund Size"
                                            amount={epochData.endOfEpochInfo?.storageFundBalance}
                                        />
                                        <TokenStats label="Net Inflow" amount={netInflow} />
                                        <TokenStats label="Fund Inflow" amount={fundInflow} />
                                        <TokenStats label="Fund Outflow" amount={fundOutflow} />
                                    </EpochStatsGrid>
                                </EpochStats>
                            </>
                        )}

                        {isCurrentEpoch && <ValidatorStatus />}
                    </div>

                    <Panel>
                        <div className="relative">
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
                        <div className="p-md">
                            {activeTabId === EpochTabs.Checkpoints ? (
                                <CheckpointsTable
                                    initialCursor={initialCursorPlusOne}
                                    maxCursor={epochData.firstCheckpointId}
                                    initialLimit={20}
                                />
                            ) : null}
                            {activeTabId === EpochTabs.Validators &&
                            committeeMembers &&
                            tableColumns ? (
                                <TableCard
                                    sortTable
                                    defaultSorting={[{ id: 'stakingPoolIotaBalance', desc: true }]}
                                    data={committeeMembers}
                                    columns={tableColumns}
                                />
                            ) : null}
                        </div>
                    </Panel>
                </div>
            }
        />
    );
}
