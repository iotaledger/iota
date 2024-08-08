// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useEffect, useState } from 'react';

export interface TableProviderProps {
    /**
     * Does the table have a checkbox column.
     */
    hasCheckboxColumn?: boolean;
    /**
     * On checkbox change callback.
     */
    onRowCheckboxChange?: (value: boolean, index: number, tableValues: boolean[]) => void;
    /**
     * On header checkbox change callback.
     */
    onHeaderCheckboxChange?: (value: boolean) => void;
}

type TableContextProps = {
    isHeaderChecked: boolean;
    toggleHeaderChecked: (checked: boolean) => void;
    rowsChecked: boolean[];
    toggleRowChecked: (checked: boolean, index: number) => boolean[];
    isHeaderIndeterminate: boolean;
    registerRowCheckbox: (index: number) => void;
};

export enum TableRowType {
    Body = 'body',
    Header = 'header',
}

export const TableContext = createContext<TableContextProps & TableProviderProps>({
    hasCheckboxColumn: false,
    isHeaderChecked: false,
    toggleHeaderChecked: () => {},
    rowsChecked: [],
    toggleRowChecked: () => [],
    isHeaderIndeterminate: false,
    onRowCheckboxChange: () => {},
    registerRowCheckbox: () => {},
});

export const useTableContext = () => {
    const context = useContext(TableContext);
    return context;
};

export function TableProvider({
    children,
    ...props
}: TableProviderProps & { children: React.ReactNode }) {
    const [rowsChecked, setRowsChecked] = useState<boolean[]>([]);
    const [isHeaderChecked, setIsHeaderChecked] = useState<boolean>(false);
    const [isHeaderIndeterminate, setIsHeaderIndeterminate] = useState<boolean>(false);

    useEffect(() => {
        if (rowsChecked.length > 0) {
            if (rowsChecked.every((checked) => checked)) {
                setIsHeaderChecked(true);
                setIsHeaderIndeterminate(false);
            } else if (rowsChecked.some((checked) => checked)) {
                setIsHeaderIndeterminate(true);
            } else {
                setIsHeaderChecked(false);
                setIsHeaderIndeterminate(false);
            }
        }
    }, [rowsChecked]);

    const toggleRowChecked = useCallback(
        (checked: boolean, index: number) => {
            const newRowsChecked = [...rowsChecked];
            newRowsChecked[index] = checked;
            setRowsChecked(newRowsChecked);
            return newRowsChecked;
        },
        [rowsChecked],
    );

    const registerRowCheckbox = useCallback((index: number) => {
        setRowsChecked((prevRowsChecked) => {
            const newRowsChecked = [...prevRowsChecked];
            newRowsChecked[index] = false;
            return newRowsChecked;
        });
    }, []);

    const toggleHeaderChecked = useCallback(
        (checked: boolean) => {
            const newCheckedRows = Array(rowsChecked.length).fill(checked);
            setIsHeaderChecked(checked);
            setRowsChecked(newCheckedRows);
        },
        [rowsChecked],
    );

    return (
        <TableContext.Provider
            value={{
                ...props,
                isHeaderChecked,
                toggleRowChecked,
                toggleHeaderChecked,
                registerRowCheckbox,
                rowsChecked,
                isHeaderIndeterminate,
            }}
        >
            {children}
        </TableContext.Provider>
    );
}
