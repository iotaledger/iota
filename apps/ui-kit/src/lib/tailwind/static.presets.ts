// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import type { ScreenBreakpoints } from './types';
import { getTailwindScreens } from './helpers';
import { BASE_CONFIG } from './base.presets';
import merge from 'lodash.merge';

const BREAKPOINTS: Partial<ScreenBreakpoints> & { default: number } = {
    default: 0,
};

const screens = getTailwindScreens(BREAKPOINTS);

const staticPreset: Config = merge({}, BASE_CONFIG, {
    theme: {
        screens,
    },
    corePlugins: {
        container: false,
    },
});

export default staticPreset;
