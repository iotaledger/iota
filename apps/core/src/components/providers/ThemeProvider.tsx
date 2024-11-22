// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren, useState, useEffect } from 'react';
import { Theme } from '../../enums';
import { ThemeContext } from '../../contexts';

interface ThemeProviderProps {
    appId: string;
}

export function ThemeProvider({ children, appId }: PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;
    const [theme, setTheme] = useState<Theme>(() => {
        if (typeof window !== 'undefined') {
            const storedTheme = localStorage.getItem(storageKey);
            if (storedTheme) {
                return storedTheme as Theme;
            } else {
                const prefersDarkScheme = window.matchMedia('(prefers-color-scheme: dark)').matches;
                return prefersDarkScheme ? Theme.Dark : Theme.Light;
            }
        }
        return Theme.Light;
    });

    useEffect(() => {
        if (typeof window !== 'undefined') {
            localStorage.setItem(storageKey, theme);
        }
        document.documentElement.classList.toggle(Theme.Dark, theme === Theme.Dark);
    }, [theme, storageKey]);

    return (
        <ThemeContext.Provider
            value={{
                theme,
                setTheme,
            }}
        >
            {children}
        </ThemeContext.Provider>
    );
}
