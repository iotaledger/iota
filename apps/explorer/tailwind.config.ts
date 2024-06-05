// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import preset from '@iota/core/tailwind.config';
import { type Config } from 'tailwindcss';

export default {
    content: ['./src/**/*.{js,jsx,ts,tsx}', './node_modules/@iota/ui/src/**/*.{js,jsx,ts,tsx}'],
    presets: [preset],
} satisfies Partial<Config>;
