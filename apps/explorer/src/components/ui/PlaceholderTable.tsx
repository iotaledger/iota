// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { TableCard } from './TableCard';
import { TableBaseCell, TableCellPlaceholder } from '@iota/apps-ui-kit';

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
        () => Object.fromEntries(colHeadings.map((index) => [`a${index}`, null])),
        [colHeadings, rowHeight],
    );

    const loadingTable = useMemo(
        () => ({
            data: new Array(rowCount).fill(rowEntry),
            columns: colHeadings.map((header) => ({
                header,
                cell: () => {
                    return (
                        <TableBaseCell isContentCentered>
                            <TableCellPlaceholder />
                        </TableBaseCell>
                    );
                },
            })),
        }),
        [rowCount, rowEntry, colHeadings],
    );

    return <TableCard data={loadingTable.data} columns={loadingTable.columns} />;
}
