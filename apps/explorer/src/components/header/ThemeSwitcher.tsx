// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType } from '@iota/apps-ui-kit';
import { DarkMode, LightMode } from '@iota/ui-icons';
import { useEffect, useLayoutEffect } from 'react';
import { useTheme, Theme } from '@iota/core';

export function ThemeSwitcher(): React.JSX.Element {
    const { theme, setTheme } = useTheme();

    const ThemeIcon = theme === Theme.Dark ? DarkMode : LightMode;

    function handleOnClick(): void {
        const newTheme = theme === Theme.Light ? Theme.Dark : Theme.Light;
        setTheme(newTheme);
        saveThemeToLocalStorage(newTheme);
    }

    function saveThemeToLocalStorage(newTheme: Theme): void {
        localStorage.setItem('theme', newTheme);
    }

    function updateDocumentClass(theme: Theme): void {
        document.documentElement.classList.toggle('dark', theme === Theme.Dark);
    }

    useLayoutEffect(() => {
        const storedTheme = localStorage.getItem('theme') as Theme | null;
        if (storedTheme) {
            setTheme(storedTheme);
            updateDocumentClass(storedTheme);
        } else {
            const prefersDarkTheme = window.matchMedia('(prefers-color-scheme: dark)').matches;
            const preferredTheme = prefersDarkTheme ? Theme.Dark : Theme.Light;

            setTheme(preferredTheme);
            updateDocumentClass(preferredTheme);
        }
    }, []);

    useEffect(() => {
        updateDocumentClass(theme);
    }, [theme]);

    return (
        <Button
            type={ButtonType.Ghost}
            onClick={handleOnClick}
            icon={<ThemeIcon className="h-5 w-5" />}
        />
    );
}
