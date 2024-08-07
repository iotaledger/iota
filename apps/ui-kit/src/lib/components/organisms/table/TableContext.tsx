// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useEffect, useState } from 'react';

type TableProps = {
    hasCheckboxColumn?: boolean;
};

type TableProviderProps = {
    headerChecked: boolean;
    toggleHeaderChecked: (checked: boolean) => void;
    rowsChecked: boolean[];
    toggleRowChecked: (index: number, checked: boolean) => void;
    isHeaderIndeterminate: boolean;
};

export enum TableRowType {
    Body = 'body',
    Header = 'header',
}

export const TableContext = createContext<TableProviderProps & TableProps>({
    hasCheckboxColumn: false,
    headerChecked: false,
    toggleHeaderChecked: () => {},
    rowsChecked: [],
    toggleRowChecked: () => {},
    isHeaderIndeterminate: false,
});

export const useTableContext = () => {
    const context = useContext(TableContext);
    return context;
};

export function TableProvider({ children, ...props }: TableProps & { children: React.ReactNode }) {
    const [headerChecked, setHeaderChecked] = useState<boolean>(false);
    const [rowsChecked, setRowsChecked] = useState<boolean[]>([]);
    const [isHeaderIndeterminate, setIsHeaderIndeterminate] = useState<boolean>(false);

    useEffect(() => {
        if (rowsChecked.length > 0) {
            const areAllChecked = rowsChecked.every((checked) => checked);
            const areSomeChecked = rowsChecked.some((checked) => checked);
            if (areAllChecked) {
                setHeaderChecked(areAllChecked);
                setIsHeaderIndeterminate(false);
            } else if (areSomeChecked) {
                setIsHeaderIndeterminate(true);
            } else {
                setHeaderChecked(false);
                setIsHeaderIndeterminate(false);
            }
        }
    }, [rowsChecked]);

    const toggleRowChecked = useCallback((index: number, checked: boolean) => {
        setRowsChecked((prevRowsChecked) => {
            const newRowsChecked = [...prevRowsChecked];
            newRowsChecked[index] = checked;
            return newRowsChecked;
        });
    }, []);

    const toggleHeaderChecked = useCallback((checked: boolean) => {
        setHeaderChecked(checked);
        setRowsChecked((prevRowsChecked) => prevRowsChecked.map(() => checked));
    }, []);

    return (
        <TableContext.Provider
            value={{
                ...props,
                headerChecked,
                toggleRowChecked,
                toggleHeaderChecked,
                rowsChecked,
                isHeaderIndeterminate,
            }}
        >
            {children}
        </TableContext.Provider>
    );
}
