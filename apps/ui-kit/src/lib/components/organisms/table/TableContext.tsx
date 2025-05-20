// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren } from 'react';
import { createContext, useContext } from 'react';

export interface TableProviderProps {
    /**
     * Numeric indexes of the selected rows.
     */
    selectedRowIndexes: Set<number>;
    /**
     * Numeric indexes of all the rows.
     */
    rowIndexes: number[];
}

type TableContextProps = {
    isHeaderChecked: boolean;
    selectedRowIndexes: Set<number>;
    isHeaderIndeterminate: boolean;
};

export const TableContext = createContext<TableContextProps>({
    isHeaderChecked: false,
    selectedRowIndexes: new Set(),
    isHeaderIndeterminate: false,
});

export const useTableContext = () => {
    const context = useContext(TableContext);
    return context;
};

export function TableProvider({
    children,
    selectedRowIndexes,
    rowIndexes,
    ...props
}: PropsWithChildren<TableProviderProps>) {
    const isHeaderChecked = rowIndexes.length === selectedRowIndexes.size;
    const isHeaderIndeterminate = !isHeaderChecked && selectedRowIndexes.size > 0;

    return (
        <TableContext.Provider
            value={{
                ...props,
                isHeaderChecked,
                selectedRowIndexes,
                isHeaderIndeterminate,
            }}
        >
            {children}
        </TableContext.Provider>
    );
}
