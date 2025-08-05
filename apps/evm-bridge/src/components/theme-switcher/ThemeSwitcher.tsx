import { DarkMode, LightMode } from '@iota/apps-ui-icons';
import { Theme, ThemePreference } from '../../lib/enums';
import { useTheme } from '../../hooks/useTheme';
import { Button, ButtonType } from '@iota/apps-ui-kit';

export function ThemeSwitcher(): React.JSX.Element {
    const { theme, themePreference, setThemePreference } = useTheme();

    const ThemeIcon = theme === Theme.Dark ? DarkMode : LightMode;

    function handleOnClick(): void {
        const newTheme =
            themePreference === ThemePreference.Light
                ? ThemePreference.Dark
                : ThemePreference.Light;
        setThemePreference(newTheme);
    }

    return (
        <Button
            onClick={handleOnClick}
            type={ButtonType.Ghost}
            icon={<ThemeIcon className="h-5 w-5" />}
        />
    );
}
