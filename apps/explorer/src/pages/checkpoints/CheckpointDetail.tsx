// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { LoadingIndicator, Text } from '@iota/ui';
import { useQuery } from '@tanstack/react-query';
import { useParams } from 'react-router-dom';

import { IotaAmount, PageLayout } from '~/components';
import {
    Banner,
    DescriptionItem,
    DescriptionList,
    EpochLink,
    PageHeader,
    TabHeader,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
} from '~/components/ui';
import { CheckpointTransactionBlocks } from './CheckpointTransactionBlocks';

export default function CheckpointDetail(): JSX.Element {
    const { id } = useParams<{ id: string }>();
    const digestOrSequenceNumber = /^\d+$/.test(id!) ? parseInt(id!, 10) : id;

    const client = useIotaClient();
    const { data, isError, isPending } = useQuery({
        queryKey: ['checkpoints', digestOrSequenceNumber],
        queryFn: () => client.getCheckpoint({ id: String(digestOrSequenceNumber!) }),
    });
    return (
        <PageLayout
            content={
                isError ? (
                    <Banner variant="error" fullWidth>
                        There was an issue retrieving data for checkpoint: {id}
                    </Banner>
                ) : isPending ? (
                    <LoadingIndicator />
                ) : (
                    <div className="flex flex-col space-y-12">
                        <PageHeader title={data.digest} type="Checkpoint" />
                        <div className="space-y-8">
                            <Tabs size="lg" defaultValue="details">
                                <TabsList>
                                    <TabsTrigger value="details">Details</TabsTrigger>
                                    <TabsTrigger value="signatures">Signatures</TabsTrigger>
                                </TabsList>
                                <TabsContent value="details">
                                    <DescriptionList>
                                        <DescriptionItem title="Checkpoint Sequence No.">
                                            <Text variant="pBody/medium" color="steel-darker">
                                                {data.sequenceNumber}
                                            </Text>
                                        </DescriptionItem>
                                        <DescriptionItem title="Epoch">
                                            <EpochLink epoch={data.epoch} />
                                        </DescriptionItem>
                                        <DescriptionItem title="Checkpoint Timestamp">
                                            <Text variant="pBody/medium" color="steel-darker">
                                                {data.timestampMs
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
                                                    : '--'}
                                            </Text>
                                        </DescriptionItem>
                                    </DescriptionList>
                                </TabsContent>
                                <TabsContent value="signatures">
                                    <Tabs defaultValue="aggregated">
                                        <TabsList>
                                            <TabsTrigger value="aggregated">
                                                Aggregated Validator Signature
                                            </TabsTrigger>
                                        </TabsList>
                                        <TabsContent value="aggregated">
                                            <DescriptionList>
                                                <DescriptionItem
                                                    key={data.validatorSignature}
                                                    title="Signature"
                                                >
                                                    <Text
                                                        variant="pBody/medium"
                                                        color="steel-darker"
                                                    >
                                                        {data.validatorSignature}
                                                    </Text>
                                                </DescriptionItem>
                                            </DescriptionList>
                                        </TabsContent>
                                    </Tabs>
                                </TabsContent>
                            </Tabs>

                            <TabHeader title="Gas & Storage Fees">
                                <DescriptionList>
                                    <DescriptionItem title="Computation Fee">
                                        <IotaAmount
                                            full
                                            amount={data.epochRollingGasCostSummary.computationCost}
                                        />
                                    </DescriptionItem>
                                    <DescriptionItem title="Storage Fee">
                                        <IotaAmount
                                            full
                                            amount={data.epochRollingGasCostSummary.storageCost}
                                        />
                                    </DescriptionItem>
                                    <DescriptionItem title="Storage Rebate">
                                        <IotaAmount
                                            full
                                            amount={data.epochRollingGasCostSummary.storageRebate}
                                        />
                                    </DescriptionItem>
                                </DescriptionList>
                            </TabHeader>

                            <TabHeader title="Checkpoint Transaction Blocks">
                                <div className="mt-4">
                                    <CheckpointTransactionBlocks id={data.sequenceNumber} />
                                </div>
                            </TabHeader>
                        </div>
                    </div>
                )
            }
        />
    );
}
