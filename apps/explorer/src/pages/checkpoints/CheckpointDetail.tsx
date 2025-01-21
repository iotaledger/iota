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
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Warning } from '@iota/apps-ui-icons';

enum FeesTabs {
    GasAndStorageFees = 'gas-and-storage-fees',
}
enum DetailsTabs {
    Details = 'details',
    Signatures = 'signatures',
}
enum NestedTabs {
    Aggregated = 'aggregated',
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

    const [formattedComputationCost, computationCostCoinType] = useFormatCoin(
        data?.epochRollingGasCostSummary?.computationCost,
        IOTA_TYPE_ARG,
    );
    const [formattedStorageCost, storageCostCoinType] = useFormatCoin(
        data?.epochRollingGasCostSummary.storageCost,
        IOTA_TYPE_ARG,
    );
    const [formattedStorageRebate, storageRebateCoinType] = useFormatCoin(
        data?.epochRollingGasCostSummary.storageRebate,
        IOTA_TYPE_ARG,
    );

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
                        <div className="flex flex-row gap-lg">
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
                                    <>
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
                                                />
                                            </SegmentedButton>
                                        </div>
                                        {activeNestedTabId === NestedTabs.Aggregated ? (
                                            <div className="flex flex-col gap-lg p-md--rs">
                                                <LabelText
                                                    size={LabelTextSize.Medium}
                                                    label="Aggregated Validator Signature"
                                                    text={data.validatorSignature}
                                                />
                                            </div>
                                        ) : null}
                                    </>
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
                                </SegmentedButton>
                                {activeFeesTabId === FeesTabs.GasAndStorageFees ? (
                                    <div className="flex flex-col gap-lg p-md--rs">
                                        <LabelText
                                            size={LabelTextSize.Medium}
                                            label="Computation Fee"
                                            text={formattedComputationCost}
                                            supportingLabel={computationCostCoinType}
                                        />
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
                                    </div>
                                ) : null}
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
