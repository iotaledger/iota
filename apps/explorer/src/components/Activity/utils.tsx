// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EpochMetricsPage } from '@iota/iota.js/client';
import { Text } from '@iota/ui';

import { IotaAmount } from '../Table/IotaAmount';
import { TxTimeType } from '../tx-time/TxTimeType';
import { HighlightedTableCol } from '~/components/Table/HighlightedTableCol';
import { CheckpointSequenceLink, EpochLink } from '~/ui/InternalLink';
import { getEpochStorageFundFlow } from '~/utils/getStorageFundFlow';

// Generate table data from the epochs data
export const genTableDataFromEpochsData = (results: EpochMetricsPage) => ({
    data: results?.data.map((epoch) => ({
        epoch: (
            <HighlightedTableCol first>
                <EpochLink epoch={epoch.epoch.toString()} />
            </HighlightedTableCol>
        ),
        transactions: <Text variant="bodySmall/medium">{epoch.epochTotalTransactions}</Text>,
        stakeRewards: <IotaAmount amount={epoch.endOfEpochInfo?.totalStakeRewardsDistributed} />,
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
});
