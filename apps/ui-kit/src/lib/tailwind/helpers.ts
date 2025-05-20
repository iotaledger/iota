// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ScreenSize } from '../enums';

export const pxToRem = (px: number, base: number = 16) => `${px / base}rem`;

export function getTailwindScreens(
    breakpoints: Partial<Record<ScreenSize, number>>,
): Record<string, string> {
    const screens: Record<string, string> = Object.entries(breakpoints).reduce(
        (acc, [key, value]) => {
            acc[key] = `${value}px`;
            return acc;
        },
        {} as Record<string, string>,
    );

    return screens;
}
