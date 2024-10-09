// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { EpochMetrics } from '@iota/iota-sdk/client';
import type { ColumnDef } from '@tanstack/react-table';
import { TableCellBase, TableCellText } from '@iota/apps-ui-kit';
import { CheckpointSequenceLink, EpochLink } from '~/components';
import { getEpochStorageFundFlow } from '~/lib/utils';

/**
 * Generate table columns renderers for the epochs data.
 */
export function generateEpochsTableColumns(currentEpoch?: string): ColumnDef<EpochMetrics>[] {
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
            cell: ({ getValue, row }) => {
                let epochTotalTransactions = getValue<EpochMetrics['epochTotalTransactions']>();
                const isCurrentEpoch = row.original.epoch === currentEpoch;
                const hasNoTransactions = !epochTotalTransactions || epochTotalTransactions === '0';
                if (isCurrentEpoch && hasNoTransactions) {
                    epochTotalTransactions = '--';
                }
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
                let totalStakeRewardsDistributed =
                    epochMetrics.endOfEpochInfo?.totalStakeRewardsDistributed;
                const isCurrentEpoch = epochMetrics.epoch === currentEpoch;
                const hasNoTransactions =
                    !totalStakeRewardsDistributed || totalStakeRewardsDistributed === '0';
                if (isCurrentEpoch && hasNoTransactions) {
                    totalStakeRewardsDistributed = '--';
                }
                return (
                    <TableCellBase>
                        <TableCellText>{totalStakeRewardsDistributed}</TableCellText>
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
                    getEpochStorageFundFlow(endOfEpochInfo).netInflow?.toString() ?? '--';
                return (
                    <TableCellBase>
                        <TableCellText>{storageNetInflow}</TableCellText>
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
                        <TableCellText>{epochEndTimestamp || '--'}</TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
