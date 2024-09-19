#! /usr/bin/env tsx
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import autoprefixer from 'autoprefixer';
import tailwindcss from 'tailwindcss';
import stylePlugin from 'esbuild-style-plugin';

import { buildPackage } from './utils/buildPackage.js';

buildPackage({
    plugins: [
        stylePlugin({
            postcss: {
                plugins: [tailwindcss, autoprefixer],
            },
        }),
    ],
    packages: 'external',
    bundle: true,
}).catch((error) => {
    console.error(error);
    process.exit(1);
});
