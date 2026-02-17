// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Configuration } from '@rspack/core';
import { merge } from 'webpack-merge';

import configCommon from './rspack.config.common';

const configDev: Configuration = {
    mode: 'development',
    devtool: 'cheap-source-map',
    watchOptions: {
        aggregateTimeout: 600,
    },
};

async function getConfig() {
    return merge(await configCommon(), configDev);
}

export default getConfig;
