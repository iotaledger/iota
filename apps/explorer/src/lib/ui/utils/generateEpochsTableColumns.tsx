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
            accessorKey: 'endOfEpochInfo.totalStakeRewardsDistributed',
            cell: ({ getValue }) => {
                const totalStakeRewardsDistributed = getValue<string | undefined>();
                return (
                    <TableCellBase>
                        <TableCellText>{totalStakeRewardsDistributed ?? '0'}</TableCellText>
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
            accessorKey: 'endOfEpochInfo.epochEndTimestamp',
            cell: ({ getValue }) => {
                const epochEndTimestamp = getValue<string>();
                return (
                    <TableCellBase>
                        <TableCellText>{epochEndTimestamp}</TableCellText>
                    </TableCellBase>
                );
            },
        },
    ];
}
