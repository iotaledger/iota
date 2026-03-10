// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { uiKitResponsivePreset } from '@iota/apps-ui-kit';
import type { Config } from 'tailwindcss';

export default {
    presets: [uiKitResponsivePreset],
    content: ['./src/**/*.{js,ts,jsx,tsx,mdx}', 'mdx-components.tsx'],
    darkMode: 'selector',
    theme: {
        extend: {},
    },
} satisfies Partial<Config>;
