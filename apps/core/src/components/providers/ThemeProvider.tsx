// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren, useState, useEffect, useCallback } from 'react';
import { Theme } from '../../enums';
import { ThemeContext } from '../../contexts';

interface ThemeProviderProps {
    appId: string;
}

export function ThemeProvider({ children, appId }: PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;

    const getSystemTheme = () =>
        window.matchMedia('(prefers-color-scheme: dark)').matches ? Theme.Dark : Theme.Light;

    const getInitialTheme = () => {
        const storedTheme = localStorage.getItem(storageKey);
        return storedTheme ? (storedTheme as Theme) : Theme.System;
    };

    const [theme, setTheme] = useState<Theme>(getInitialTheme);

    const applyTheme = useCallback((currentTheme: Theme) => {
        const selectedTheme = currentTheme === Theme.System ? getSystemTheme() : currentTheme;
        const documentElement = document.documentElement.classList;
        documentElement.toggle(Theme.Dark, selectedTheme === Theme.Dark);
        documentElement.toggle(Theme.Light, selectedTheme === Theme.Light);
    }, []);

    useEffect(() => {
        localStorage.setItem(storageKey, theme);
        applyTheme(theme);

        if (theme === Theme.System) {
            const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');
            const handleSystemThemeChange = () => applyTheme(Theme.System);
            systemTheme.addEventListener('change', handleSystemThemeChange);
            return () => systemTheme.removeEventListener('change', handleSystemThemeChange);
        }
    }, [theme, applyTheme, storageKey]);

    return <ThemeContext.Provider value={{ theme, setTheme }}>{children}</ThemeContext.Provider>;
}
