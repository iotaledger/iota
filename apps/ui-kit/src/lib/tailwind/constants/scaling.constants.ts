// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { pxToRem } from '../helpers';

type TailwindScale = Record<string, string> & { DEFAULT?: string };

export const BORDER_RADIUS: TailwindScale = {
    none: pxToRem(0),
    sm: pxToRem(2),
    DEFAULT: pxToRem(4),
    md: pxToRem(6),
    lg: pxToRem(8),
    xl: pxToRem(12),
    '2xl': pxToRem(16),
    '3xl': pxToRem(24),
    full: pxToRem(120),
};

export const CUSTOM_SPACING: TailwindScale = {
    none: pxToRem(0),
    xxxs: pxToRem(2),
    xxs: pxToRem(4),
    xs: pxToRem(8),
    sm: pxToRem(12),
    md: pxToRem(16),
    lg: pxToRem(24),
    xl: pxToRem(32),
    '2xl': pxToRem(48),
};

export const OPACITY: TailwindScale = {
    0: '0',
    8: '0.08',
    12: '0.12',
    16: '0.16',
    40: '0.4',
    60: '0.6',
    80: '0.8',
    100: '1',
};
