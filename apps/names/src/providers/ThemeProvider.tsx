// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { UIKitTheme } from '@iota/apps-ui-kit';
import { useEffect, useState } from 'react';

import { ThemeContext } from '@/contexts';

import { ThemePreference } from '../lib/enums';

interface ThemeProviderProps {
    appId?: string;
    staticTheme?: UIKitTheme;
}

export function ThemeProvider({
    children,
    appId,
    staticTheme,
}: React.PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;

    const getSystemTheme = () => {
        return window.matchMedia('(prefers-color-scheme: dark)').matches
            ? UIKitTheme.Dark
            : UIKitTheme.Light;
    };

    const getThemePreference = () => {
        const storedTheme = localStorage?.getItem(storageKey) as ThemePreference | null;
        return storedTheme ? storedTheme : ThemePreference.System;
    };

    const [systemTheme, setSystemTheme] = useState<UIKitTheme>(staticTheme ?? UIKitTheme.Light);
    const [themePreference, setThemePreference] = useState<ThemePreference>(ThemePreference.System);
    const [isLoadingPreference, setIsLoadingPreference] = useState(true);

    // Load the theme values on client
    useEffect(() => {
        if (typeof window === 'undefined') return;

        setSystemTheme(getSystemTheme());

        if (!staticTheme) {
            setThemePreference(getThemePreference());
        }

        // Make the theme preference listener wait
        // until the preference is loaded in the next render
        setIsLoadingPreference(false);
    }, []);

    // When the theme preference changes..
    useEffect(() => {
        if (typeof window === 'undefined' || isLoadingPreference || !!staticTheme) return;

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
    }, [themePreference, storageKey, isLoadingPreference, staticTheme]);

    // Derive the active theme from the preference
    const theme = (() => {
        if (staticTheme) return staticTheme;

        switch (themePreference) {
            case ThemePreference.Dark:
                return UIKitTheme.Dark;
            case ThemePreference.Light:
                return UIKitTheme.Light;
            case ThemePreference.System:
                return systemTheme;
            case ThemePreference.Names:
                return UIKitTheme.Names;
        }
    })();

    // When the theme (preference or derived) changes update the CSS class
    useEffect(() => {
        if (staticTheme) return;

        const documentElement = document.documentElement.classList;
        documentElement.toggle(UIKitTheme.Dark, theme === UIKitTheme.Dark);
        documentElement.toggle(UIKitTheme.Light, theme === UIKitTheme.Light);
        documentElement.toggle(UIKitTheme.Names, theme === UIKitTheme.Names);
    }, [theme]);

    return (
        <ThemeContext.Provider value={{ theme, setThemePreference, themePreference }}>
            {children}
        </ThemeContext.Provider>
    );
}
