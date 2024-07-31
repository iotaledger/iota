// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ScreenSize } from '../../enums/screenSize.enum';

enum ResponsiveSpacingKeys {
    Xs = 'xs--rs',
    Sm = 'sm--rs',
    Md = 'md--rs',
}

type DefaultValue = string;
type ScreenMdValue = string;
type ScreenLgValue = string;
type ScreenXlValue = string;

const VARIABLE_SPACING_EXTRA_BREAKPOINTS = [ScreenSize.Md, ScreenSize.Lg, ScreenSize.Xl];
const VARIABLE_SPACING: Record<
    ResponsiveSpacingKeys,
    [DefaultValue, ScreenMdValue, ScreenLgValue?, ScreenXlValue?]
> = {
    [ResponsiveSpacingKeys.Xs]: ['4px', '8px'],
    [ResponsiveSpacingKeys.Sm]: ['8px', '12px', '16px', '24px'],
    [ResponsiveSpacingKeys.Md]: ['16px', '24px'],
};

function getClampedValues(key: ResponsiveSpacingKeys, screens: Record<string, string>): string {
    const values = VARIABLE_SPACING[key];
    let clampValue = `${values[0]}`;
    for (let i = 1; i < values.length; i++) {
        if (values[i]) {
            const breakpoint = screens[VARIABLE_SPACING_EXTRA_BREAKPOINTS[i - 1]];
            if (breakpoint) {
                clampValue = `clamp(${clampValue}, (100vw - ${breakpoint}) * 99, ${values[i]})`;
            }
        }
    }

    return clampValue;
}

export function getResponsiveSpacing(themeScreens: Record<string, string>) {
    const spacing = Object.keys(VARIABLE_SPACING).reduce((acc, _key) => {
        const key = _key as ResponsiveSpacingKeys;
        const values = {
            ...acc,
            [key]: getClampedValues(key, themeScreens),
        };
        return values;
    }, {});
    return spacing;
}
