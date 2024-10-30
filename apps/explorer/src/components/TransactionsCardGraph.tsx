// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatAmount, formatBalance, formatDate } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { LabelTextSize, TooltipPosition } from '@iota/apps-ui-kit';
import { GraphTooltip, StatisticsPanel } from './StatisticsPanel';

interface TooltipContentProps {
    data: {
        epochTotalTransactions: number;
        epochStartTimestamp: number;
        epoch: number;
    };
}

function TooltipContent({
    data: { epochTotalTransactions, epochStartTimestamp, epoch },
}: TooltipContentProps): JSX.Element {
    const dateFormatted = formatDate(new Date(epochStartTimestamp), ['day', 'month']);
    const totalFormatted = formatAmount(epochTotalTransactions);

    const overline = `${dateFormatted}, Epoch ${epoch}`;

    return (
        <GraphTooltip overline={overline} title={totalFormatted} subtitle="Transaction Blocks" />
    );
}

function useEpochTransactions() {
    return useIotaClientQuery(
        'getEpochMetrics',
        {
            descendingOrder: true,
            limit: 31,
        },
        {
            select: (data) =>
                data.data
                    .map(({ epoch, epochTotalTransactions, epochStartTimestamp }) => ({
                        epoch: Number(epoch),
                        epochTotalTransactions: Number(epochTotalTransactions),
                        epochStartTimestamp: Number(epochStartTimestamp),
                    }))
                    .reverse()
                    .slice(0, -1),
        },
    );
}

export function TransactionsCardGraph() {
    const { data: totalTransactions } = useIotaClientQuery(
        'getTotalTransactionBlocks',
        {},
        {
            gcTime: 24 * 60 * 60 * 1000,
            staleTime: Infinity,
            retry: 5,
        },
    );
    const { data: epochMetrics, isPending } = useEpochTransactions();

    const lastEpochTotalTransactions =
        epochMetrics?.[epochMetrics.length - 1]?.epochTotalTransactions;

    const lastEpochTotalTransactionsFormatted = lastEpochTotalTransactions
        ? formatBalance(lastEpochTotalTransactions, 0, CoinFormat.ROUNDED)
        : '--';

    const stats: React.ComponentProps<typeof StatisticsPanel>['stats'] = [
        {
            size: LabelTextSize.Large,
            label: 'Total',
            text: totalTransactions ? formatBalance(totalTransactions, 0) : '--',
            tooltipPosition: TooltipPosition.Right,
            tooltipText: 'The total number of transaction blocks.',
        },
        {
            size: LabelTextSize.Large,
            label: 'Last epoch',
            text: lastEpochTotalTransactionsFormatted,
        },
    ];

    return (
        <StatisticsPanel
            title="Transaction Blocks"
            data={epochMetrics}
            stats={stats}
            isPending={isPending}
            getX={({ epoch }) => Number(epoch)}
            getY={({ epochTotalTransactions }) => Number(epochTotalTransactions)}
            formatY={formatAmount}
            tooltipContent={TooltipContent}
        />
    );
}
