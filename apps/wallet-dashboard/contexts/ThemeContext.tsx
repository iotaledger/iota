// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';

export enum Theme {
    Light = 'light',
    Dark = 'dark',
}

interface ThemeContextType {
    theme: Theme;
    toggleTheme: () => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [theme, setTheme] = useState<Theme.Light | Theme.Dark>(Theme.Light);

    useEffect(() => {
        document.documentElement.classList.toggle(Theme.Dark, theme === Theme.Dark);
    }, [theme]);

    const toggleTheme = () => {
        setTheme((prevTheme) => (prevTheme === Theme.Dark ? Theme.Light : Theme.Dark));
    };

    return <ThemeContext.Provider value={{ theme, toggleTheme }}>{children}</ThemeContext.Provider>;
};

export const useTheme = (): ThemeContextType => {
    const context = useContext(ThemeContext);
    if (!context) {
        throw new Error('useTheme must be used within a ThemeProvider');
    }
    return context;
};
