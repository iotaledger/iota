// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import preset from '@iota/core/tailwind.config';
import { type Config } from 'tailwindcss';
import { uiKitStaticPreset } from '@iota/apps-ui-kit';

export default {
    presets: [preset, uiKitStaticPreset],
    content: [
        './src/**/*.{js,jsx,ts,tsx}',
        './node_modules/@iota/ui/src/**/*.{js,jsx,ts,tsx}',
        './node_modules/@iota/apps-ui-kit/**/*.js',
    ],
    theme: {
        screens: {
            sm: '600px',
            md: '905px',
            lg: '1240kpx',
            xl: '1440px',
        },
    },
} satisfies Partial<Config>;
