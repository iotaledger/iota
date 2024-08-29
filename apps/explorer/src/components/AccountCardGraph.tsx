// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAmount, formatDate } from '@iota/core';
import { type AllEpochsAddressMetrics } from '@iota/iota-sdk/client';
import { Heading, LoadingIndicator, Text } from '@iota/ui';
import { ParentSize } from '@visx/responsive';
import { useMemo } from 'react';

import { AreaGraph } from './AreaGraph';
import { ErrorBoundary } from './error-boundary/ErrorBoundary';
import { useGetAddressMetrics } from '~/hooks/useGetAddressMetrics';
import { useGetAllEpochAddressMetrics } from '~/hooks/useGetAllEpochAddressMetrics';
import { LabelText, LabelTextSize, Panel, Title, TitleSize } from '@iota/apps-ui-kit';

const GRAPH_DATA_FIELD = 'cumulativeAddresses';
const GRAPH_DATA_TEXT = 'Total accounts';

function TooltipContent({ data }: { data: AllEpochsAddressMetrics[number] }): JSX.Element {
    const dateFormatted = formatDate(new Date(data.timestampMs), ['day', 'month']);
    const totalFormatted = formatAmount(data[GRAPH_DATA_FIELD]);
    return (
        <div className="flex flex-col gap-0.5">
            <Text variant="subtitleSmallExtra/medium" color="steel-darker">
                {dateFormatted}, Epoch {data.epoch}
            </Text>
            <Heading variant="heading6/semibold" color="steel-darker">
                {totalFormatted}
            </Heading>
            <Text variant="subtitleSmallExtra/medium" color="steel-darker" uppercase>
                {GRAPH_DATA_TEXT}
            </Text>
        </div>
    );
}

export function AccountsCardGraph(): JSX.Element {
    const { data: addressMetrics } = useGetAddressMetrics();
    const { data: allEpochMetrics, isPending } = useGetAllEpochAddressMetrics({
        descendingOrder: false,
    });
    const adjEpochAddressMetrics = useMemo(() => allEpochMetrics?.slice(-30), [allEpochMetrics]);
    return (
        <Panel>
            <Title title="Accounts" size={TitleSize.Medium} />
            <div className="flex h-full flex-col gap-md p-md--rs">
                <div className="flex flex-row gap-md">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total"
                            text={
                                addressMetrics?.cumulativeAddresses
                                    ? addressMetrics.cumulativeAddresses.toString()
                                    : '--'
                            }
                            showSupportingLabel={false}
                        />
                    </div>

                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Active"
                            text={
                                addressMetrics?.cumulativeActiveAddresses
                                    ? addressMetrics.cumulativeActiveAddresses.toString()
                                    : '--'
                            }
                            showSupportingLabel={false}
                        />
                    </div>
                </div>
                <LabelText
                    size={LabelTextSize.Large}
                    label="Daily Active"
                    text={
                        addressMetrics?.dailyActiveAddresses
                            ? addressMetrics.dailyActiveAddresses.toString()
                            : '--'
                    }
                    showSupportingLabel={false}
                />
                <div className="flex min-h-[180px] flex-1 flex-col items-center justify-center rounded-xl transition-colors">
                    {isPending ? (
                        <div className="flex flex-col items-center gap-1">
                            <LoadingIndicator />
                            <Text color="steel" variant="body/medium">
                                loading data
                            </Text>
                        </div>
                    ) : adjEpochAddressMetrics?.length ? (
                        <div className="relative flex-1 self-stretch">
                            <ErrorBoundary>
                                <ParentSize className="absolute">
                                    {({ height, width }) => (
                                        <AreaGraph
                                            data={adjEpochAddressMetrics}
                                            height={height}
                                            width={width}
                                            getX={({ epoch }) => epoch}
                                            getY={(data) => data[GRAPH_DATA_FIELD]}
                                            color="blue"
                                            formatY={formatAmount}
                                            tooltipContent={TooltipContent}
                                        />
                                    )}
                                </ParentSize>
                            </ErrorBoundary>
                        </div>
                    ) : (
                        <Text color="steel" variant="body/medium">
                            No historical data available
                        </Text>
                    )}
                </div>
            </div>
        </Panel>
    );
}
