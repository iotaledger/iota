// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { EpochMetrics } from '@iota/iota-sdk/client';
import { ColumnDef } from '@tanstack/react-table';
import { TableBaseCell, TableCellText } from '@iota/apps-ui-kit';
import { CheckpointLink, CheckpointSequenceLink } from '~/components';
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
                    <TableBaseCell isContentCentered>
                        <TableCellText>{epoch}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Transaction Blocks',
            accessorKey: 'epochTotalTransactions',
            cell: ({ getValue }) => {
                const epochTotalTransactions = getValue<EpochMetrics['epochTotalTransactions']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{epochTotalTransactions}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Stake Rewards',
            accessorKey: 'stakeRewards.endOfEpochInfo.totalStakeRewardsDistributed',
            cell: ({ getValue }) => {
                const totalStakeRewardsDistributed = getValue<string | undefined>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{totalStakeRewardsDistributed ?? '0'}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Checkpoint Set',
            accessorKey: 'firstCheckpointId',
            cell: ({ getValue }) => {
                const firstCheckpointId = getValue<EpochMetrics['firstCheckpointId']>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>
                            <CheckpointSequenceLink sequence={firstCheckpointId}>
                                {firstCheckpointId}
                            </CheckpointSequenceLink>
                        </TableCellText>
                    </TableBaseCell>
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
                    <TableBaseCell isContentCentered>
                        <TableCellText>{storageNetInflow}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
        {
            header: 'Epoch End',
            accessorKey: 'endOfEpochInfo.epochEndTimestamp',
            cell: ({ getValue }) => {
                const epochEndTimestamp = getValue<string>();
                return (
                    <TableBaseCell isContentCentered>
                        <TableCellText>{epochEndTimestamp}</TableCellText>
                    </TableBaseCell>
                );
            },
        },
    ];
}
