// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatAmount, formatBalance, formatDate } from '@iota/core';
import { type AllEpochsAddressMetrics } from '@iota/iota-sdk/client';
import { ParentSize } from '@visx/responsive';
import { useMemo } from 'react';
import { AreaGraph } from './AreaGraph';
import { ErrorBoundary } from './error-boundary/ErrorBoundary';
import { useGetAddressMetrics } from '~/hooks/useGetAddressMetrics';
import { useGetAllEpochAddressMetrics } from '~/hooks/useGetAllEpochAddressMetrics';
import {
    LabelText,
    LabelTextSize,
    LoadingIndicator,
    Panel,
    Title,
    TitleSize,
    TooltipPosition,
} from '@iota/apps-ui-kit';

const GRAPH_DATA_FIELD = 'cumulativeAddresses';
const GRAPH_DATA_TEXT = 'Total addresses';

function TooltipContent({ data }: { data: AllEpochsAddressMetrics[number] }): JSX.Element {
    const dateFormatted = formatDate(new Date(data.timestampMs), ['day', 'month']);
    const totalFormatted = formatAmount(data[GRAPH_DATA_FIELD]);
    return (
        <div className="flex flex-col gap-0.5">
            <span className="text-body-sm text-neutral-40">
                {dateFormatted}, Epoch {data.epoch}
            </span>
            <span className="text-label-lg text-neutral-12">{totalFormatted}</span>
            <span className="text-body-sm text-neutral-40">{GRAPH_DATA_TEXT}</span>
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
                            tooltipPosition={TooltipPosition.Right}
                            tooltipText="The total amount of addresses that have been part of transactions since the network started."
                        />
                    </div>

                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Active"
                            text={cumulativeActiveAddressesFormatted}
                            tooltipPosition={TooltipPosition.Right}
                            tooltipText="The total number of addresses that have signed transactions since the network started."
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
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="The total number of addresses that have sent or received transactions during the last epoch."
                />
                <div className="flex min-h-[180px] flex-1 flex-col items-center justify-center rounded-xl transition-colors">
                    {isPending ? (
                        <LoadingIndicator text="Loading data" />
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
                        <div className="flex items-center justify-center">
                            <span className="flex flex-row items-center gap-x-xs text-neutral-40 dark:text-neutral-60">
                                No historical data available
                            </span>
                        </div>
                    )}
                </div>
            </div>
        </Panel>
    );
}
