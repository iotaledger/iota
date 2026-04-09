// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType } from '@iota/apps-ui-kit';
import { DarkMode, LightMode } from '@iota/apps-ui-icons';
import { Theme, ThemePreference } from '../../enums';
import { useTheme } from '../../hooks';

interface ThemeSwitcherProps {
    onThemeChange?: (theme: ThemePreference) => void;
}

export function ThemeSwitcher({ onThemeChange }: ThemeSwitcherProps = {}): React.JSX.Element {
    const { theme, themePreference, setThemePreference } = useTheme();

    const ThemeIcon = theme === Theme.Dark ? DarkMode : LightMode;

    function handleOnClick(): void {
        const newTheme =
            themePreference === ThemePreference.Light
                ? ThemePreference.Dark
                : ThemePreference.Light;
        setThemePreference(newTheme);
        onThemeChange?.(newTheme);
    }

    return (
        <Button
            type={ButtonType.Ghost}
            onClick={handleOnClick}
            icon={<ThemeIcon className="h-5 w-5" />}
            aria-label={theme === Theme.Dark ? 'Switch to light mode' : 'Switch to dark mode'}
        />
    );
}
