// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EpochMetrics } from '@iota/iota-sdk/client';
import type { ColumnDef } from '@tanstack/react-table';
import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import { CheckpointSequenceLink, EpochLink } from '~/components';
import { getEpochStorageFundFlow } from '~/lib/utils';
import { getElapsedTime } from '~/pages/epochs/utils';
import { CoinFormat, formatBalance } from '@iota/core';
import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';

/**
 * Generate table columns renderers for the epochs data.
 */
export function generateEpochsTableColumns(): ColumnDef<EpochMetrics>[] {
    return [
        {
            header: 'Epoch',
            accessorKey: 'epoch',
            cell: ({ getValue }) => {
                const epoch = getValue<EpochMetrics['epoch']>();
                return (
                    <TableCellBase>
                        <EpochLink epoch={epoch}>
                            <TableCellText>{epoch}</TableCellText>
                        </EpochLink>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Transaction Blocks',
            accessorKey: 'epochTotalTransactions',
            cell: ({ getValue }) => {
                const epochTotalTransactions = getValue<EpochMetrics['epochTotalTransactions']>();
                return (
                    <TableCellBase>
                        <TableCellText>{epochTotalTransactions}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Stake Rewards',
            id: 'stakeRewards',
            accessorKey: 'endOfEpochInfo.totalStakeRewardsDistributed',
            cell: ({ row: { original: epochMetrics } }) => {
                const totalStakeRewardsDistributed =
                    epochMetrics.endOfEpochInfo?.totalStakeRewardsDistributed;
                const totalStakeRewardsDistributedFormatted = totalStakeRewardsDistributed
                    ? formatBalance(Number(totalStakeRewardsDistributed) / Number(NANOS_PER_IOTA), 0, CoinFormat.ROUNDED)
                    : '--';
                return (
                    <TableCellBase>
                        <TableCellText supportingLabel={totalStakeRewardsDistributed ? 'IOTA' : undefined}>
                            {totalStakeRewardsDistributedFormatted ?? '0'}
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Checkpoint Set',
            accessorKey: 'firstCheckpointId',
            cell: ({ getValue }) => {
                const firstCheckpointId = getValue<EpochMetrics['firstCheckpointId']>();
                return (
                    <TableCellBase>
                        <TableCellText>
                            <CheckpointSequenceLink sequence={firstCheckpointId}>
                                {firstCheckpointId}
                            </CheckpointSequenceLink>
                        </TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Storage Net Inflow',
            accessorKey: 'endOfEpochInfo',
            cell: ({ getValue }) => {
                const endOfEpochInfo = getValue<EpochMetrics['endOfEpochInfo']>();
                const storageNetInflow =
                    getEpochStorageFundFlow(endOfEpochInfo).netInflow;
                const storageNetInflowFormatted = storageNetInflow
                    ? formatBalance(storageNetInflow / NANOS_PER_IOTA, 0, CoinFormat.ROUNDED)
                    : '--';
                return (
                    <TableCellBase>
                        <TableCellText supportingLabel={storageNetInflow ? 'IOTA' : undefined}>{storageNetInflowFormatted}</TableCellText>
                    </TableCellBase>
                );
            },
        },
        {
            header: 'Epoch End',
            id: 'epochEndTimestamp',
            cell: ({ row: { original: epochMetrics } }) => {
                const epochEndTimestamp = epochMetrics.endOfEpochInfo?.epochEndTimestamp;
                return (
                    <TableCellBase>
                        <TableCellText>{epochEndTimestamp ? getElapsedTime(Number(epochEndTimestamp), Date.now()) : '--'}</TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
