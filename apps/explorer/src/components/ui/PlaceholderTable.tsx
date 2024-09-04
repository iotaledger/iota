// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Placeholder } from '@iota/ui';
import { useMemo } from 'react';

import { TableCard } from './TableCard';
import { TableCellType } from '@iota/apps-ui-kit';

export interface PlaceholderTableProps {
    rowCount: number;
    rowHeight: string;
    colHeadings: string[];
}

export function PlaceholderTable({
    rowCount,
    rowHeight,
    colHeadings,
}: PlaceholderTableProps): JSX.Element {
    const rowEntry = useMemo(
        () =>
            Object.fromEntries(
                colHeadings.map((header, index) => [
                    `a${index}`,
                    {
                        type: TableCellType.Placeholder,
                    },
                ]),
            ),
        [colHeadings, rowHeight],
    );

    const loadingTable = useMemo(
        () => ({
            data: new Array(rowCount).fill(rowEntry),
            columns: colHeadings.map((header, index) => ({
                header: header,
                accessorKey: `a${index}`,
            })),
        }),
        [rowCount, rowEntry, colHeadings],
    );

    return <TableCard data={loadingTable.data} columns={loadingTable.columns} />;
}
