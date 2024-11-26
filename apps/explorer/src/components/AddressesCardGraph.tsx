// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatAmount, formatBalance, formatDate } from '@iota/core';
import type { AllEpochsAddressMetrics } from '@iota/iota-sdk/client';
import { useMemo } from 'react';
import { useGetAddressMetrics } from '~/hooks/useGetAddressMetrics';
import { useGetAllEpochAddressMetrics } from '~/hooks/useGetAllEpochAddressMetrics';
import { LabelTextSize, TooltipPosition } from '@iota/apps-ui-kit';
import { StatisticsPanel, GraphTooltip } from './StatisticsPanel';

const GRAPH_DATA_FIELD = 'cumulativeAddresses';
const GRAPH_DATA_TEXT = 'Total addresses';

function TooltipContent({ data }: { data: AllEpochsAddressMetrics[number] }): JSX.Element {
    const dateFormatted = formatDate(new Date(data.timestampMs), ['day', 'month']);
    const totalFormatted = formatAmount(data[GRAPH_DATA_FIELD]);

    const overline = `${dateFormatted}, Epoch ${data.epoch}`;
    return <GraphTooltip overline={overline} title={totalFormatted} subtitle={GRAPH_DATA_TEXT} />;
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

    const stats: React.ComponentProps<typeof StatisticsPanel>['stats'] = [
        {
            size: LabelTextSize.Large,
            label: 'Total',
            text: cumulativeAddressesFormatted,
            tooltipPosition: TooltipPosition.Right,
            tooltipText:
                'The total amount of addresses that have been part of transactions since the network started.',
        },
        {
            size: LabelTextSize.Large,
            label: 'Total Active',
            text: cumulativeActiveAddressesFormatted,
            tooltipPosition: TooltipPosition.Right,
            tooltipText:
                'The total number of addresses that have signed transactions since the network started.',
        },
        {
            size: LabelTextSize.Large,
            label: 'Daily Active',
            text: addressMetrics?.dailyActiveAddresses
                ? addressMetrics.dailyActiveAddresses.toString()
                : '--',
            tooltipPosition: TooltipPosition.Right,
            tooltipText:
                'The total number of addresses that have sent or received transactions during the last epoch.',
        },
    ];
    return (
        <StatisticsPanel
            title="Addresses"
            data={adjEpochAddressMetrics}
            stats={stats}
            isPending={isPending}
            getX={({ epoch }) => Number(epoch) || 0}
            getY={(data) => Number(data[GRAPH_DATA_FIELD]) || 0}
            formatY={formatAmount}
            tooltipContent={TooltipContent}
        />
    );
}
