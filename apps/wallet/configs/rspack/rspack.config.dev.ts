// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Configuration } from '@rspack/core';

const configDev: Configuration = {
    extends: './configs/rspack/rspack.config.common.ts',
    mode: 'development',
    devtool: 'cheap-source-map',
    watchOptions: {
        aggregateTimeout: 600,
    },
};

export default configDev;
