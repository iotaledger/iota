// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EpochMetricsPage } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';

import { IotaAmount, TxTimeType, HighlightedTableCol } from '~/components';
import { CheckpointSequenceLink, EpochLink } from '~/components/ui';
import { getEpochStorageFundFlow } from '~/lib/utils';
import { type ReactNode } from 'react';

interface EpochData {
    epoch: ReactNode;
    transactions: ReactNode;
    stakeRewards: ReactNode;
    checkpointSet: ReactNode;
    storageNetInflow: ReactNode;
    time: ReactNode;
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
            epoch: (
                <HighlightedTableCol first>
                    <EpochLink epoch={epoch.epoch.toString()} />
                </HighlightedTableCol>
            ),
            transactions: <Text variant="bodySmall/medium">{epoch.epochTotalTransactions}</Text>,
            stakeRewards: (
                <IotaAmount amount={epoch.endOfEpochInfo?.totalStakeRewardsDistributed} />
            ),
            checkpointSet: (
                <div>
                    <CheckpointSequenceLink sequence={epoch.firstCheckpointId.toString()} />
                    {` - `}
                    <CheckpointSequenceLink
                        sequence={epoch.endOfEpochInfo?.lastCheckpointId.toString() ?? ''}
                    />
                </div>
            ),
            storageNetInflow: (
                <div className="pl-3">
                    <IotaAmount amount={getEpochStorageFundFlow(epoch.endOfEpochInfo).netInflow} />
                </div>
            ),
            time: <TxTimeType timestamp={Number(epoch.endOfEpochInfo?.epochEndTimestamp ?? 0)} />,
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
