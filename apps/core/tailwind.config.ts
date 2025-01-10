// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Config } from 'tailwindcss';
import colors from 'tailwindcss/colors';
import uiKitResponsivePreset from '../../apps/ui-kit/src/lib/tailwind/responsive.presets';

export default {
    presets: [uiKitResponsivePreset],
    content: ['./src/**/*.{js,jsx,ts,tsx}', './../ui-kit/src/lib/**/*.{js,jsx,ts,tsx}'],
    theme: {
        extend: {
            colors: {
                white: colors.white,
                black: colors.black,
                transparent: colors.transparent,
                inherit: colors.inherit,
            },
        },
    },
} satisfies Partial<Config>;
