// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { UIKitTheme } from '@iota/apps-ui-kit';
import { createContext } from 'react';

import { ThemePreference } from '../lib/enums';

export interface ThemeContextType {
    theme: UIKitTheme;
    themePreference: ThemePreference;
    setThemePreference: (theme: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextType>({
    theme: UIKitTheme.Names,
    themePreference: ThemePreference.System,
    setThemePreference: () => {},
});
