// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren, useState, useEffect } from 'react';
import { Theme, ThemePreference } from '../../enums';
import { ThemeContext } from '../../contexts';

const THEME_TO_HTML_CLASS: Record<Theme, string> = {
    [Theme.Dark]: 'dark',
    [Theme.Light]: 'light',
    [Theme.Names]: 'dark names',
};

interface ThemeProviderProps {
    appId: string;
    staticTheme?: Theme;
}

export function ThemeProvider({
    children,
    appId,
    staticTheme,
}: PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;

    const getSystemTheme = () => {
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? Theme.Dark : Theme.Light;
    };

    const getThemePreference = () => {
        const storedTheme = localStorage?.getItem(storageKey) as ThemePreference | null;
        return storedTheme ? storedTheme : ThemePreference.System;
    };

    const [systemTheme, setSystemTheme] = useState<Theme>(staticTheme ?? Theme.Light);
    const [themePreference, setThemePreference] = useState<ThemePreference>(ThemePreference.System);
    const [isLoadingPreference, setIsLoadingPreference] = useState(true);

    // Load the theme values on client
    useEffect(() => {
        if (typeof window === 'undefined') return;

        setSystemTheme(getSystemTheme());
        setThemePreference(getThemePreference());

        // Make the theme preference listener wait
        // until the preference is loaded in the next render
        setIsLoadingPreference(false);
    }, []);

    // When the theme preference changes..
    useEffect(() => {
        if (typeof window === 'undefined' || isLoadingPreference) return;

        // Update localStorage with the new preference
        localStorage.setItem(storageKey, themePreference);

        if (!staticTheme) {
            // In case of SystemPreference, listen for system theme changes
            if (themePreference === ThemePreference.System) {
                const handleSystemThemeChange = () => {
                    const systemTheme = getSystemTheme();
                    setSystemTheme(systemTheme);
                };
                const systemThemeMatcher = window.matchMedia('(prefers-color-scheme: dark)');
                systemThemeMatcher.addEventListener('change', handleSystemThemeChange);
                return () =>
                    systemThemeMatcher.removeEventListener('change', handleSystemThemeChange);
            }
        }
    }, [themePreference, storageKey, isLoadingPreference, staticTheme]);

    // Derive the active theme from the preference
    const selectedTheme = (() => {
        if (staticTheme) return staticTheme;

        switch (themePreference) {
            case ThemePreference.Dark:
                return Theme.Dark;
            case ThemePreference.Light:
                return Theme.Light;
            case ThemePreference.System:
                return systemTheme;
            case ThemePreference.Names:
                return Theme.Names;
        }
    })();

    // When the theme (preference or derived) changes update the CSS class
    useEffect(() => {
        Object.values(THEME_TO_HTML_CLASS).forEach((classes) => {
            for (const className of classes.split(' ')) {
                document.documentElement.classList.remove(className);
            }
        });

        const activeClasses = THEME_TO_HTML_CLASS[selectedTheme];
        for (const className of activeClasses.split(' ')) {
            document.documentElement.classList.add(className);
        }
    }, [selectedTheme]);

    return (
        <ThemeContext.Provider
            value={{ theme: selectedTheme, setThemePreference, themePreference }}
        >
            {children}
        </ThemeContext.Provider>
    );
}
