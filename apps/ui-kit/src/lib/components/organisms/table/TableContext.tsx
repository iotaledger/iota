// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext } from 'react';

type TableContextType = {
    hasCheckboxColumn: boolean;
};

export const TableContext = createContext<TableContextType>({ hasCheckboxColumn: false });

export const useTableContext = () => {
    const context = useContext(TableContext);
    return context;
};
