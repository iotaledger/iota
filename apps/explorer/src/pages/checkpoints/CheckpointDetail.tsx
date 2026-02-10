// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { useParams } from 'react-router-dom';
import { PageLayout, PageHeader } from '~/components';
import { CheckpointTransactionBlocks } from './CheckpointTransactionBlocks';
import {
    ButtonSegment,
    ButtonSegmentType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LabelText,
    LabelTextSize,
    LoadingIndicator,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
} from '@iota/apps-ui-kit';
import { useState } from 'react';
import { useFormatCoin } from '@iota/core';
import { Info, Warning } from '@iota/apps-ui-icons';
import { CoinFormat } from '@iota/iota-sdk/utils';

enum FeesTabs {
    GasAndStorageFees = 'gas-and-storage-fees',
    RollingGasAndStorageFees = 'rolling-gas-and-storage-fees',
}
enum DetailsTabs {
    Details = 'details',
    Signatures = 'signatures',
}
enum NestedTabs {
    Aggregated = 'aggregated',
}

type CheckpointFeeDetails = {
    computationCost?: string;
    computationCostBurned?: string;
    storageCost?: string;
    storageRebate?: string;
};

function calculateDifference(a?: string, b?: string): string {
    const res = BigInt(a ?? '0') - BigInt(b ?? '0');
    return (res < 0n ? 0n : res).toString();
}

function getCheckpointFeesFromRolling(
    current?: CheckpointFeeDetails,
    prev?: CheckpointFeeDetails,
): CheckpointFeeDetails | undefined {
    if (!current) return undefined;
    return {
        computationCost: calculateDifference(current.computationCost, prev?.computationCost),
        computationCostBurned: calculateDifference(
            current.computationCostBurned,
            prev?.computationCostBurned,
        ),
        storageCost: calculateDifference(current.storageCost, prev?.storageCost),
        storageRebate: calculateDifference(current.storageRebate, prev?.storageRebate),
    };
}

export function CheckpointDetail(): JSX.Element {
    const [activeFeesTabId, setActiveFeesTabId] = useState(FeesTabs.GasAndStorageFees);
    const [activeDetailsTabId, setActiveDetailsTabId] = useState(DetailsTabs.Details);
    const [activeNestedTabId, setActiveNestedTabId] = useState(NestedTabs.Aggregated);

    const { id } = useParams<{ id: string }>();
    const digestOrSequenceNumber = /^\d+$/.test(id!) ? parseInt(id!, 10) : id;

    const client = useIotaClient();
    const { data, isError, isPending } = useQuery({
        queryKey: ['checkpoints', digestOrSequenceNumber],
        queryFn: () => client.getCheckpoint({ id: String(digestOrSequenceNumber!) }),
    });

    const previousDigest = data?.previousDigest;

    const { data: previousCheckpointData } = useQuery({
        queryKey: ['checkpoints', previousDigest],
        queryFn: () => client.getCheckpoint({ id: String(previousDigest!) }),
        enabled: !!previousDigest && !isPending && !isError,
    });

    const epochRollingGasCostSummary: CheckpointFeeDetails | undefined =
        data?.epochRollingGasCostSummary;

    const previousRollingEpochGasSummary: CheckpointFeeDetails | undefined =
        previousCheckpointData && data && previousCheckpointData.epoch === data.epoch
            ? previousCheckpointData.epochRollingGasCostSummary
            : undefined;

    const checkpointFeeSummary = getCheckpointFeesFromRolling(
        epochRollingGasCostSummary,
        previousRollingEpochGasSummary,
    );

    const selectedFeeSummary =
        activeFeesTabId === FeesTabs.GasAndStorageFees
            ? checkpointFeeSummary
            : epochRollingGasCostSummary;

    const [formattedComputationCost, computationCostCoinType] = useFormatCoin({
        balance: selectedFeeSummary?.computationCost,
        format: CoinFormat.Full,
    });
    const [formattedComputationCostBurned, computationCostBurnedCoinType] = useFormatCoin({
        balance: selectedFeeSummary?.computationCostBurned,
        format: CoinFormat.Full,
    });
    const [formattedStorageCost, storageCostCoinType] = useFormatCoin({
        balance: selectedFeeSummary?.storageCost,
        format: CoinFormat.Full,
    });
    const [formattedStorageRebate, storageRebateCoinType] = useFormatCoin({
        balance: selectedFeeSummary?.storageRebate,
        format: CoinFormat.Full,
    });

    return (
        <PageLayout
            content={
                isError ? (
                    <InfoBox
                        title="Failed to load checkpoint data"
                        supportingText={`There was an issue retrieving data for checkpoint: ${id}`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : isPending ? (
                    <LoadingIndicator />
                ) : (
                    <div className="flex flex-col gap-2xl">
                        <PageHeader title={data.digest} type="Checkpoint" />
                        <div className="flex flex-col gap-lg md:flex-row">
                            <Panel>
                                <SegmentedButton
                                    type={SegmentedButtonType.Transparent}
                                    shape={ButtonSegmentType.Underlined}
                                >
                                    <ButtonSegment
                                        type={ButtonSegmentType.Underlined}
                                        label="Details"
                                        selected={activeDetailsTabId === DetailsTabs.Details}
                                        onClick={() => setActiveDetailsTabId(DetailsTabs.Details)}
                                    />
                                    <ButtonSegment
                                        type={ButtonSegmentType.Underlined}
                                        label="Signatures"
                                        selected={activeDetailsTabId === DetailsTabs.Signatures}
                                        onClick={() =>
                                            setActiveDetailsTabId(DetailsTabs.Signatures)
                                        }
                                    />
                                </SegmentedButton>
                                {activeDetailsTabId === DetailsTabs.Details ? (
                                    <div className="flex flex-col gap-lg p-md--rs">
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Checkpoint Sequence No."
                                            text={data.sequenceNumber}
                                        />
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Epoch"
                                            text={data.epoch}
                                        />
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Checkpoint Timestamp"
                                            text={
                                                data.timestampMs
                                                    ? new Date(
                                                          Number(data.timestampMs),
                                                      ).toLocaleString(undefined, {
                                                          month: 'short',
                                                          day: 'numeric',
                                                          year: 'numeric',
                                                          hour: 'numeric',
                                                          minute: '2-digit',
                                                          second: '2-digit',
                                                          hour12: false,
                                                          timeZone: 'UTC',
                                                          timeZoneName: 'short',
                                                      })
                                                    : '--'
                                            }
                                        />
                                    </div>
                                ) : null}
                                {activeDetailsTabId === DetailsTabs.Signatures ? (
                                    <div className="flex flex-wrap p-sm--rs">
                                        <div className="inline-flex">
                                            <SegmentedButton
                                                type={SegmentedButtonType.Transparent}
                                                shape={ButtonSegmentType.Underlined}
                                            >
                                                <ButtonSegment
                                                    type={ButtonSegmentType.Underlined}
                                                    label="Aggregated Validator Signature"
                                                    selected={
                                                        activeNestedTabId === NestedTabs.Aggregated
                                                    }
                                                    onClick={() =>
                                                        setActiveNestedTabId(NestedTabs.Aggregated)
                                                    }
                                                    isNested
                                                />
                                            </SegmentedButton>
                                        </div>
                                        {activeNestedTabId === NestedTabs.Aggregated ? (
                                            <div className="flex flex-col gap-lg break-all p-md--rs">
                                                <LabelText
                                                    size={LabelTextSize.Medium}
                                                    label="Aggregated Validator Signature"
                                                    text={data.validatorSignature}
                                                />
                                            </div>
                                        ) : null}
                                    </div>
                                ) : null}
                            </Panel>
                            <Panel>
                                <SegmentedButton
                                    type={SegmentedButtonType.Transparent}
                                    shape={ButtonSegmentType.Underlined}
                                >
                                    <ButtonSegment
                                        type={ButtonSegmentType.Underlined}
                                        label="Gas & Storage Fees"
                                        selected={activeFeesTabId === FeesTabs.GasAndStorageFees}
                                        onClick={() =>
                                            setActiveFeesTabId(FeesTabs.GasAndStorageFees)
                                        }
                                    />
                                    <ButtonSegment
                                        type={ButtonSegmentType.Underlined}
                                        label="Rolling Gas & Storage Fees"
                                        selected={
                                            activeFeesTabId === FeesTabs.RollingGasAndStorageFees
                                        }
                                        onClick={() =>
                                            setActiveFeesTabId(FeesTabs.RollingGasAndStorageFees)
                                        }
                                    />
                                </SegmentedButton>

                                <div className="flex flex-col gap-lg p-md--rs">
                                    <div className="flex flex-row items-center gap-lg">
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Computation Fee"
                                            text={formattedComputationCost}
                                            supportingLabel={computationCostCoinType}
                                        />
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Burnt"
                                            text={formattedComputationCostBurned}
                                            supportingLabel={computationCostBurnedCoinType}
                                        />
                                    </div>

                                    <LabelText
                                        size={LabelTextSize.Medium}
                                        label="Storage Fee"
                                        text={formattedStorageCost}
                                        supportingLabel={storageCostCoinType}
                                    />
                                    <LabelText
                                        size={LabelTextSize.Medium}
                                        label="Storage Rebate"
                                        text={formattedStorageRebate}
                                        supportingLabel={storageRebateCoinType}
                                    />
                                    {activeFeesTabId === FeesTabs.RollingGasAndStorageFees ? (
                                        <InfoBox
                                            title="Fees of all transactions included in the current epoch
                                            so far until this checkpoint"
                                            icon={<Info />}
                                            type={InfoBoxType.Default}
                                            style={InfoBoxStyle.Elevated}
                                        />
                                    ) : null}
                                    {activeFeesTabId === FeesTabs.GasAndStorageFees ? (
                                        <InfoBox
                                            title="Gas and storage fees for this checkpoint only"
                                            icon={<Info />}
                                            type={InfoBoxType.Default}
                                            style={InfoBoxStyle.Elevated}
                                        />
                                    ) : null}
                                </div>
                            </Panel>
                        </div>
                        <Panel>
                            <Title title="Checkpoint Transaction Blocks" />
                            <div className="p-md--rs">
                                <CheckpointTransactionBlocks id={data.sequenceNumber} />
                            </div>
                        </Panel>
                    </div>
                )
            }
        />
    );
}
