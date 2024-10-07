// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatAmount, formatBalance, formatDate } from '@iota/core';
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
const GRAPH_DATA_TEXT = 'Total addresses';

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

const FALLBACK = '--';

export function AddressesCardGraph(): JSX.Element {
    const { data: addressMetrics } = useGetAddressMetrics();
    const { data: allEpochMetrics, isPending } = useGetAllEpochAddressMetrics({
        descendingOrder: false,
    });
    const adjEpochAddressMetrics = useMemo(() => allEpochMetrics?.slice(-30), [allEpochMetrics]);

    const cumulativeAddressesFormatted = addressMetrics?.cumulativeAddresses
        ? formatBalance(addressMetrics.cumulativeAddresses, 0, CoinFormat.ROUNDED)
        : FALLBACK;

    const cumulativeActiveAddressesFormatted = addressMetrics?.cumulativeActiveAddresses
        ? formatBalance(addressMetrics.cumulativeActiveAddresses, 0, CoinFormat.ROUNDED)
        : FALLBACK;

    return (
        <Panel>
            <Title title="Addresses" size={TitleSize.Medium} />
            <div className="flex h-full flex-col gap-md p-md--rs">
                <div className="flex flex-row gap-md">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total"
                            text={cumulativeAddressesFormatted}
                        />
                    </div>

                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Active"
                            text={cumulativeActiveAddressesFormatted}
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
                                            getX={({ epoch }) => Number(epoch) || 0}
                                            getY={(data) => Number(data[GRAPH_DATA_FIELD]) || 0}
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
