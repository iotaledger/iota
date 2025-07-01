import { useState, useEffect, useCallback } from 'react';
import { Theme, ThemePreference } from '../lib/enums';
import { ThemeContext } from '../contexts';

interface ThemeProviderProps {
    appId: string;
}

export function ThemeProvider({ children, appId }: React.PropsWithChildren<ThemeProviderProps>) {
    const storageKey = `theme_${appId}`;

    const getSystemTheme = () => {
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? Theme.Dark : Theme.Light;
    };

    const getThemePreference = useCallback(() => {
        const storedTheme = localStorage?.getItem(storageKey) as ThemePreference | null;
        return storedTheme ? storedTheme : ThemePreference.System;
    }, [storageKey]);

    const [systemTheme, setSystemTheme] = useState<Theme>(Theme.Light);
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
    }, [getThemePreference]);

    // When the theme preference changes..
    useEffect(() => {
        if (typeof window === 'undefined' || isLoadingPreference) return;

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
    }, [themePreference, storageKey, isLoadingPreference]);

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
