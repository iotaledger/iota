// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useState } from 'react';

export interface TableProviderProps {
    /**
     * On checkbox change callback.
     */
    onRowCheckboxChange?: (value: boolean, index: number, tableValues: Set<number>) => void;
    /**
     * On header checkbox change callback.
     */
    onHeaderCheckboxChange?: (value: boolean) => void;
    /**
     * Numeric indexes of all the rows.
     */
    rowIndexes: number[];
}

type TableContextProps = {
    onRowCheckboxChange?: (value: boolean, index: number, tableValues: boolean[]) => void;
    isHeaderChecked: boolean;
    toggleHeaderChecked: (checked: boolean) => void;
    rowsChecked: Set<number>;
    toggleRowChecked: (checked: boolean, index: number) => void;
    isHeaderIndeterminate: boolean;
};

export enum TableRowType {
    Body = 'body',
    Header = 'header',
}

export const TableContext = createContext<TableContextProps>({
    isHeaderChecked: false,
    toggleHeaderChecked: () => {},
    rowsChecked: new Set(),
    toggleRowChecked: () => new Set(),
    isHeaderIndeterminate: false,
});

export const useTableContext = () => {
    const context = useContext(TableContext);
    return context;
};

export function TableProvider({
    children,
    onHeaderCheckboxChange,
    onRowCheckboxChange,
    rowIndexes,
    ...props
}: TableProviderProps & { children: React.ReactNode }) {
    const [rowsChecked, setRowsChecked] = useState<Set<number>>(new Set());
    const isHeaderChecked = rowIndexes.length === rowsChecked.size;
    const isHeaderIndeterminate = !isHeaderChecked && rowsChecked.size > 0;

    const toggleRowChecked = useCallback(
        (checked: boolean, index: number) => {
            const newRowsChecked = new Set(rowsChecked);
            if (checked) {
                newRowsChecked.add(index);
            } else {
                newRowsChecked.delete(index);
            }
            setRowsChecked(newRowsChecked);
            onRowCheckboxChange?.(checked, index, newRowsChecked);
        },
        [rowsChecked],
    );

    const toggleHeaderChecked = useCallback(
        (checked: boolean) => {
            let newCheckedRows = new Set<number>();
            if (checked) {
                newCheckedRows = new Set(rowIndexes);
            }
            setRowsChecked(newCheckedRows);
            onHeaderCheckboxChange?.(checked);
        },
        [rowsChecked],
    );

    return (
        <TableContext.Provider
            value={{
                ...props,
                isHeaderChecked,
                toggleHeaderChecked,
                toggleRowChecked,
                rowsChecked,
                isHeaderIndeterminate,
            }}
        >
            {children}
        </TableContext.Provider>
    );
}
