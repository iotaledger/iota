// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { TableCard } from './TableCard';

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
                    0,
                ]),
            ),
        [colHeadings, rowHeight],
    );

    const loadingTable = useMemo(
        () => ({
            data: new Array(rowCount).fill(rowEntry),
            columns: colHeadings.map((header, index) => ({
                header: header,
                cell: () => {
                    return <span>loading</span>
                }
            })),
        }),
        [rowCount, rowEntry, colHeadings],
    );

    console.log("placeholder",loadingTable)

    return <TableCard data={loadingTable.data} columns={loadingTable.columns} />;
}
