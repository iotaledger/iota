// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SCREEN_BREAKPOINTS } from '.';
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

const VARIABLE_SPACING: Record<
    ResponsiveSpacingKeys,
    [DefaultValue, ScreenMdValue, ScreenLgValue?, ScreenXlValue?]
> = {
    [ResponsiveSpacingKeys.Xs]: ['4px', '8px'],
    [ResponsiveSpacingKeys.Sm]: ['8px', '12px'],
    [ResponsiveSpacingKeys.Md]: ['16px', '24px'],
};

export const responsiveSpacing = Object.keys(VARIABLE_SPACING).reduce((acc, _key) => {
    const key = _key as ResponsiveSpacingKeys;
    return {
        ...acc,
        [key]: getClampedValues(key),
    };
}, {});

function getClampedValues(key: ResponsiveSpacingKeys): string {
    const values = VARIABLE_SPACING[key];
    const breakpoints = [ScreenSize.Md, ScreenSize.Lg, ScreenSize.Xl];

    let clampValue = `${values[0]}`;
    for (let i = 1; i < values.length; i++) {
        if (values[i]) {
            const breakpoint = SCREEN_BREAKPOINTS[breakpoints[i - 1]];
            clampValue = `clamp(${clampValue}, (100vw - ${breakpoint}px) * 99, ${values[i]})`;
        }
    }

    return clampValue;
}
