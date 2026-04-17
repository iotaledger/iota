// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';

export const GLOBAL_DATE_TYPE = 'global' as const;

export type DateType =
    | 'transaction'
    | 'epoch'
    | 'checkpoint'
    | 'package'
    | 'table'
    | 'graph'
    | typeof GLOBAL_DATE_TYPE;

export type DateFormat = 'default' | 'local' | 'utc';

const LS_KEY = 'timeFormat';
const CYCLE: DateFormat[] = ['default', 'local', 'utc'];
const DEFAULT_FORMAT: DateFormat = 'default';

type DateFormatMap = Partial<Record<DateType, DateFormat>>;

interface DateFormatContextValue {
    formats: DateFormatMap;
    cycle: (type: DateType) => void;
}

function readFromStorage(): DateFormatMap {
    try {
        const raw = localStorage.getItem(LS_KEY);
        return raw ? JSON.parse(raw) : {};
    } catch {
        return {};
    }
}

function writeToStorage(map: DateFormatMap): void {
    try {
        localStorage.setItem(LS_KEY, JSON.stringify(map));
    } catch (e) {
        // storage unavailable (private mode, quota exceeded)
    }
}

const DateFormatContext = createContext<DateFormatContextValue>({
    formats: {},
    cycle: () => {},
});

export function DateFormatProvider({ children }: { children: ReactNode }): JSX.Element {
    const [formats, setFormats] = useState<DateFormatMap>(() => readFromStorage());

    const cycle = useCallback((type: DateType) => {
        setFormats((prev) => {
            const current = prev[type] ?? DEFAULT_FORMAT;
            const next = CYCLE[(CYCLE.indexOf(current) + 1) % CYCLE.length];
            const updated = { ...prev, [type]: next };
            writeToStorage(updated);
            return updated;
        });
    }, []);

    return (
        <DateFormatContext.Provider value={{ formats, cycle }}>
            {children}
        </DateFormatContext.Provider>
    );
}

export function useDateFormat(type: DateType): { format: DateFormat; cycle: () => void } {
    const { formats, cycle } = useContext(DateFormatContext);
    return {
        format: formats[type] ?? DEFAULT_FORMAT,
        cycle: () => cycle(type),
    };
}
