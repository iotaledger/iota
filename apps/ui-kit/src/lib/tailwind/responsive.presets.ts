// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import type { ScreenBreakpoints } from './types';
import { ScreenSize } from '../enums';
import { getTailwindScreens, pxToRem } from './helpers';
import { BASE_CONFIG } from './base.presets';
import merge from 'lodash.merge';

const BREAKPOINTS: ScreenBreakpoints = {
    [ScreenSize.Sm]: 768,
    [ScreenSize.Md]: 1024,
    [ScreenSize.Lg]: 1400,
    [ScreenSize.Xl]: 1920,
};

const screens = getTailwindScreens(BREAKPOINTS);

const responsivePreset: Config = merge({}, BASE_CONFIG, {
    theme: {
        screens,
        container: {
            center: true,
            screens,
            padding: {
                DEFAULT: pxToRem(24),
                md: pxToRem(48),
                lg: pxToRem(120),
                xl: pxToRem(240),
            },
        },
    },
});

export default responsivePreset;
