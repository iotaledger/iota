// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EpochMetricsPage } from '@iota/iota-sdk/client';
import { getEpochStorageFundFlow } from '~/lib/utils';
import { type TableCellProps, TableCellType } from '@iota/apps-ui-kit';
import { checkpointSequenceToLink, epochToLink } from '~/components';

interface EpochData {
    epoch: TableCellProps;
    transactions: TableCellProps;
    stakeRewards: TableCellProps;
    checkpointSet: TableCellProps;
    storageNetInflow: TableCellProps;
    time: TableCellProps;
}

interface TableColumn {
    header: string;
    accessorKey: keyof EpochData;
}

interface EpochTableData {
    data: EpochData[];
    columns: TableColumn[];
}

// Generate table data from the epochs data
export function generateTableDataFromEpochsData(results: EpochMetricsPage): EpochTableData {
    return {
        data: results?.data.map((epoch) => ({
            epoch: {
                type: TableCellType.Link,
                label: epoch.epoch,
                to: epochToLink({ epoch: epoch.epoch }),
            },
            transactions: { type: TableCellType.Text, label: epoch.epochTotalTransactions },
            stakeRewards: {
                type: TableCellType.Text,
                label: epoch.endOfEpochInfo?.totalStakeRewardsDistributed ?? '0',
            },
            checkpointSet: {
                type: TableCellType.Link,
                label: epoch.firstCheckpointId,
                to: checkpointSequenceToLink({ sequence: epoch.firstCheckpointId }),
            },
            storageNetInflow: {
                type: TableCellType.Text,
                label: getEpochStorageFundFlow(epoch.endOfEpochInfo).netInflow?.toString() ?? '--',
            },
            time: {
                type: TableCellType.Text,
                label: epoch.endOfEpochInfo?.epochEndTimestamp ?? '--',
            },
        })),
        columns: [
            {
                header: 'Epoch',
                accessorKey: 'epoch',
            },
            {
                header: 'Transaction Blocks',
                accessorKey: 'transactions',
            },
            {
                header: 'Stake Rewards',
                accessorKey: 'stakeRewards',
            },
            {
                header: 'Checkpoint Set',
                accessorKey: 'checkpointSet',
            },
            {
                header: 'Storage Net Inflow',
                accessorKey: 'storageNetInflow',
            },
            {
                header: 'Epoch End',
                accessorKey: 'time',
            },
        ],
    };
}
