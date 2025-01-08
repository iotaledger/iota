// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ScreenSize } from '../../enums/screenSize.enums';

enum VariableSpacingClassPrefix {
    Xs = 'xs--rs',
    Sm = 'sm--rs',
    Md = 'md--rs',
}

type DefaultValue = string;
type ScreenMdValue = string;
type ScreenLgValue = string;
type ScreenXlValue = string;

const VARIABLE_SPACING: Record<
    VariableSpacingClassPrefix,
    [DefaultValue, ScreenMdValue, ScreenLgValue?, ScreenXlValue?]
> = {
    [VariableSpacingClassPrefix.Xs]: ['4px', '8px'],
    [VariableSpacingClassPrefix.Sm]: ['8px', '12px'],
    [VariableSpacingClassPrefix.Md]: ['16px', '24px'],
};

function getClampedSpacingValues(
    key: VariableSpacingClassPrefix,
    screens: Record<string, string>,
): string {
    const CUSTOM_SCREEN_SIZES: ScreenSize[] = [
        ScreenSize.Sm,
        ScreenSize.Md,
        ScreenSize.Lg,
        ScreenSize.Xl,
    ];

    const values = VARIABLE_SPACING[key];
    // Use the first value as the default value, since it doesn't need to be clamped.
    let clampValue = `${values[0]}`;
    for (let i = 1; i < values.length; i++) {
        if (values[i]) {
            // Iterate the tailwind screens and check if desired the breakpoint exists.
            const breakpoint = screens[CUSTOM_SCREEN_SIZES[i]];
            if (breakpoint) {
                // Clamp the spacing value using the screen breakpoint as reference.
                clampValue = `clamp(${clampValue}, (100vw - ${breakpoint}) * 99, ${values[i]})`;
            }
        }
    }

    return clampValue;
}

export function generateVariableSpacing(tailwindScreens: Record<string, string>) {
    const variableSpacing = Object.keys(VARIABLE_SPACING).reduce((acc, variableSpacingPrefix) => {
        const clampedSpacing = getClampedSpacingValues(
            variableSpacingPrefix as VariableSpacingClassPrefix,
            tailwindScreens,
        );
        return {
            ...acc,
            [variableSpacingPrefix]: clampedSpacing,
        };
    }, {});
    return variableSpacing;
}
