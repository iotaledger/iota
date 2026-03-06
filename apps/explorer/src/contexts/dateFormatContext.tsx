// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';

export type DateType = 'transaction' | 'epoch' | 'checkpoint' | 'package' | 'table' | 'graph';
export type DateFormat = 'relative' | 'absolute';

type DateFormatMap = Record<DateType, DateFormat>;

interface DateFormatContextValue {
    formats: DateFormatMap;
    toggle: (type: DateType) => void;
}

const DEFAULT_FORMATS: DateFormatMap = {
    transaction: 'relative',
    epoch: 'relative',
    checkpoint: 'relative',
    package: 'relative',
    table: 'relative',
    graph: 'relative',
};

const DateFormatContext = createContext<DateFormatContextValue>({
    formats: DEFAULT_FORMATS,
    toggle: () => {},
});

export function DateFormatProvider({ children }: { children: ReactNode }): JSX.Element {
    const [formats, setFormats] = useState<DateFormatMap>(DEFAULT_FORMATS);

    const toggle = useCallback((type: DateType) => {
        setFormats((prev) => ({
            ...prev,
            [type]: prev[type] === 'relative' ? 'absolute' : 'relative',
        }));
    }, []);

    return (
        <DateFormatContext.Provider value={{ formats, toggle }}>
            {children}
        </DateFormatContext.Provider>
    );
}

export function useDateFormat(type: DateType): { format: DateFormat; toggle: () => void } {
    const { formats, toggle } = useContext(DateFormatContext);
    return {
        format: formats[type],
        toggle: () => toggle(type),
    };
}
