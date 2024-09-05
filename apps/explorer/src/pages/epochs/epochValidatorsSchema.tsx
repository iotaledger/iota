// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { ColumnDef } from '@tanstack/react-table';

export const validatorColumns: ColumnDef<object, unknown>[] = [
    {
        header: 'Name',
        accessorKey: 'name',
    },
    {
        header: 'Stake',
        accessorKey: 'stake',
    },
    {
        header: 'Proposed next Epoch gas price',
        accessorKey: 'nextEpochGasPrice',
    },
    {
        header: 'APY',
        accessorKey: 'apy',
    },
    {
        header: 'Commission',
        accessorKey: 'commission',
    },
    {
        header: 'Last Epoch Reward',
        accessorKey: 'lastReward',
    },
    {
        header: 'Voting Power',
        accessorKey: 'votingPower',
    },
    {
        header: 'Status',
        accessorKey: 'atRisk',
    },
];
