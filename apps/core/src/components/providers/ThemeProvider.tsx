// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren, useState, useEffect } from 'react';
import { Theme, ThemePreference } from '../../enums';
import { ThemeContext } from '../../contexts';

interface ThemeProviderProps {
    appId: string;
}

export function ThemeProvider({ children, appId }: PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;

    const getSystemTheme = () => {
        if (typeof window === 'undefined') return Theme.Light;
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? Theme.Dark : Theme.Light;
    };

    const getThemePreference = () => {
        if (typeof window === 'undefined') {
            return ThemePreference.System;
        } else {
            const storedTheme = localStorage?.getItem(storageKey) as ThemePreference | null;
            return storedTheme ? storedTheme : ThemePreference.System;
        }
    };

    const [systemTheme, setSystemTheme] = useState<Theme>(getSystemTheme);
    const [themePreference, setThemePreference] = useState<ThemePreference>(getThemePreference);

    // When the theme preference changes..
    useEffect(() => {
        if (typeof window === 'undefined') return;

        // Update localStorage with the new preference
        localStorage.setItem(storageKey, themePreference);

        // In case of SystemPreference, listen for system theme changes
        if (themePreference === ThemePreference.System) {
            const handleSystemThemeChange = () => {
                const systemTheme = getSystemTheme();
                setSystemTheme(systemTheme);
            };
            const systemThemeMatcher = window.matchMedia('(prefers-color-scheme: dark)');
            systemThemeMatcher.addEventListener('change', handleSystemThemeChange);
            return () => systemThemeMatcher.removeEventListener('change', handleSystemThemeChange);
        }
    }, [themePreference, storageKey]);

    // Derive the active theme from the preference
    const theme = (() => {
        switch (themePreference) {
            case ThemePreference.Dark:
                return Theme.Dark;
            case ThemePreference.Light:
                return Theme.Light;
            case ThemePreference.System:
                return systemTheme;
        }
    })();

    // When the theme (preference or derived) changes update the CSS class
    useEffect(() => {
        const documentElement = document.documentElement.classList;
        documentElement.toggle(Theme.Dark, theme === Theme.Dark);
        documentElement.toggle(Theme.Light, theme === Theme.Light);
    }, [theme]);

    return (
        <ThemeContext.Provider value={{ theme, setThemePreference, themePreference }}>
            {children}
        </ThemeContext.Provider>
    );
}
