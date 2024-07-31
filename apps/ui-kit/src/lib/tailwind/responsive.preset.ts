// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import { getTailwindScreens, pxToRem } from './helpers';
import { BASE_CONFIG } from './base.preset';
import merge from 'lodash.merge';
import { SCREEN_BREAKPOINTS } from './constants';

const screens = getTailwindScreens(SCREEN_BREAKPOINTS);

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
