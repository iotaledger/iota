import { createContext } from 'react';
import { Theme, ThemePreference } from '../lib/enums';

export interface ThemeContextType {
    theme: Theme;
    themePreference: ThemePreference;
    setThemePreference: (theme: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextType>({
    theme: Theme.Light,
    themePreference: ThemePreference.System,
    setThemePreference: () => {},
});
