// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from 'vitest/config';

export default defineConfig({
    resolve: {
        alias: {
            '@iota/bcs': new URL('../bcs/src', import.meta.url).toString(),
            '@iota/iota.js': new URL('../typescript/src', import.meta.url).toString(),
        },
    },
});
